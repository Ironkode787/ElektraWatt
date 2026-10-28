// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas model through the shell on a temporary copy of Golden A (WP-S2.02 part 1, acceptance checks A1, A2,
//! A5, A8 and the model half of A9).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_viewmodel::actions::{self, ACTIONS, Menu, Submenu};
use ew_viewmodel::canvas::{CanvasBackend, ZOOM_MAX, ZOOM_MIN, rect_centre};
use ew_viewmodel::script::{self, NoExtension};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-vm-canvas-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    /// A headless shell with Golden A (a copy) open read-only on folio 103.
    fn shell_on_103(&self) -> ShellModel {
        let dir = self.0.join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &dir);
        let mut shell = ShellModel::new(
            Mode::Headless,
            ShellConfig::in_dir(&self.0.join("config"), "0.0.0-test"),
        );
        done(
            &mut shell,
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": true}),
        );
        done(&mut shell, "page.goto", json!({"folio": "103"}));
        shell
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[track_caller]
fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

fn mm(nm: i64) -> f64 {
    nm as f64 / 1e6
}

/// The `state` of a transcript step for the current shell.
fn state(shell: &ShellModel) -> Value {
    serde_json::from_str(&script::state_json(shell)).unwrap()
}

/// Whether the menu item `id` (a top-level item of a menu) is checked.
fn checked(shell: &ShellModel, id: &str) -> bool {
    actions::menu_model(shell)
        .iter()
        .find_map(|m| m.items().into_iter().find(|i| i.id == id).map(|i| i.checked))
        .unwrap()
}

#[test]
fn a2_the_canvas_actions_are_in_the_registry_with_their_menus_and_reasons() {
    for id in [
        "view.zoom_in",
        "view.zoom_out",
        "view.zoom_actual",
        "view.fit_page",
        "view.pan_left",
        "view.pan_right",
        "view.pan_up",
        "view.pan_down",
        "view.grid",
        "view.frame_stats",
        "view.cpu_canvas",
        "view.zoom_to",
        "view.pan",
        "view.canvas_resize",
        "canvas.pointer",
        "canvas.snapshot",
    ] {
        let meta = actions::find(id).unwrap_or_else(|| panic!("{id} is registered"));
        assert!(ew_viewmodel::strings::lookup(meta.label_key).is_some(), "{id}");
        let hidden = matches!(
            id,
            "view.zoom_to" | "view.pan" | "view.canvas_resize" | "canvas.pointer"
        );
        assert_eq!(meta.hidden, hidden, "{id}");
        assert_eq!(meta.headless_only, id == "canvas.snapshot", "{id}");
        assert_eq!(meta.menu.is_none(), hidden || id == "canvas.snapshot", "{id}");
    }
    let keys = |id: &str| actions::find(id).unwrap().shortcut.to_vec();
    assert_eq!(keys("view.zoom_in"), ["Ctrl+Plus", "Ctrl+Equals"]);
    assert_eq!(keys("view.zoom_out"), ["Ctrl+Minus"]);
    assert_eq!(keys("view.zoom_actual"), ["Ctrl+0"]);
    assert_eq!(keys("view.fit_page"), ["Home"]);
    assert_eq!(keys("view.pan_right"), ["Ctrl+Arrow Right"]);
    // No shortcut is bound twice within a shell context (WP-S2.01 A1; a context's binding may shadow a global
    // one, WP-S2.03 §6.6) and none is Ctrl+Alt (the registry test of WP-S2.01 covers the rest).
    let all: Vec<(actions::Context, &str)> = ACTIONS
        .iter()
        .flat_map(|a| a.shortcut.iter().map(move |s| (a.context, *s)))
        .collect();
    for (i, (c, s)) in all.iter().enumerate() {
        let clash = all[i + 1..].iter().any(|(c2, s2)| s2 == s && c2 == c);
        assert!(!clash, "{s}");
        assert!(!(s.contains("Ctrl") && s.contains("Alt")), "{s}");
    }
    // View holds the pan submenu, and the grid and statistics items are checked while on.
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    let view = actions::menu_model(&shell)
        .into_iter()
        .find(|m| m.menu == Menu::View)
        .unwrap();
    // WP-S2.17 part 2: under the Zoom group.
    let pan: Vec<&str> = view
        .submenu(Submenu::Pan)
        .expect("View has the Pan submenu")
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(
        pan,
        ["view.pan_left", "view.pan_right", "view.pan_up", "view.pan_down"]
    );
    assert!(checked(&shell, "view.grid"));
    assert!(!checked(&shell, "view.frame_stats"));
    done(&mut shell, "view.grid", json!({}));
    done(&mut shell, "view.frame_stats", json!({}));
    assert!(!checked(&shell, "view.grid"));
    assert!(checked(&shell, "view.frame_stats"));
    // The refusals at the zoom ends.
    done(&mut shell, "view.zoom_to", json!({"percent": 5000}));
    assert_eq!(shell.canvas().unwrap().zoom(), ZOOM_MAX);
    assert_eq!(
        shell.dispatch("view.zoom_in", &json!({})),
        Outcome::Refused {
            reason_key: "reason-zoom-max"
        }
    );
    done(&mut shell, "view.zoom_to", json!({"percent": 1}));
    assert_eq!(shell.canvas().unwrap().zoom(), ZOOM_MIN, "clamped to 5 %");
    assert_eq!(
        shell.dispatch("view.zoom_out", &json!({})),
        Outcome::Refused {
            reason_key: "reason-zoom-min"
        }
    );
    // Without a page every canvas action but the pane size and the statistics is refused.
    done(&mut shell, "file.close", json!({}));
    assert!(shell.canvas().is_none());
    assert!(matches!(
        shell.dispatch("view.zoom_in", &json!({})),
        Outcome::Refused { .. }
    ));
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 900, "height": 700, "ppp": 1.5}),
    );
    assert_eq!(shell.canvas_pane(), ((900.0, 700.0), 1.5));
    done(&mut shell, "view.frame_stats", json!({}));
    shell.shutdown();
}

#[test]
fn a1_zoom_pan_fit_and_resize_through_the_shell() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    let c = shell.canvas().expect("folio 103 is on the canvas");
    assert_eq!(c.page().folio, "103");
    assert_eq!(c.sheet(), (297_000_000, 210_000_000));
    assert_eq!(c.pane(), (1200.0, 800.0));
    assert!(c.is_fitted());
    let fit = c.zoom();
    done(&mut shell, "view.zoom_in", json!({}));
    assert_eq!(shell.canvas().unwrap().zoom(), 1.0);
    done(&mut shell, "view.zoom_actual", json!({}));
    assert_eq!(shell.canvas().unwrap().zoom(), 1.0);
    done(&mut shell, "view.pan_right", json!({}));
    // One eighth of 1200 points at 100 % is 39.6875 mm.
    assert_eq!(shell.canvas().unwrap().centre().0, 148_500_000 + 39_687_500);
    done(&mut shell, "view.pan_down", json!({}));
    assert_eq!(shell.canvas().unwrap().centre().1, 105_000_000 + 26_458_333);
    done(&mut shell, "view.pan", json!({"dx": 96, "dy": 0}));
    assert_eq!(
        shell.canvas().unwrap().centre().0,
        148_500_000 + 39_687_500 - 25_400_000
    );
    // Zoom about an anchor keeps it in place.
    done(
        &mut shell,
        "view.zoom_to",
        json!({"percent": 400, "x_mm": 100.0, "y_mm": 80.0}),
    );
    assert_eq!(shell.canvas().unwrap().zoom(), 4.0);
    let before = shell.canvas().unwrap().centre();
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 1000, "height": 600, "ppp": 2}),
    );
    assert_eq!(
        shell.canvas().unwrap().centre(),
        before,
        "a resize keeps the centre"
    );
    assert_eq!(shell.canvas().unwrap().zoom(), 4.0, "and the zoom");
    done(&mut shell, "view.fit_page", json!({}));
    let refit = shell.canvas().unwrap().zoom();
    assert!(refit < fit, "the smaller pane fits at a smaller zoom");
    // A page change fits the new page; the pane stays.
    done(&mut shell, "page.next", json!({}));
    let c = shell.canvas().unwrap();
    assert_ne!(c.page().folio, "103");
    assert!(c.is_fitted());
    assert_eq!(c.pane(), (1000.0, 600.0));
    // Bad parameters fail without changing the view.
    for (action, params) in [
        ("view.zoom_to", json!({})),
        ("view.zoom_to", json!({"percent": -5})),
        ("view.zoom_to", json!({"percent": 100, "x_mm": 1})),
        ("view.pan", json!({"dx": "far", "dy": 0})),
        ("view.canvas_resize", json!({"width": 0, "height": 10})),
        ("canvas.pointer", json!({"x_mm": 3})),
    ] {
        assert!(
            matches!(shell.dispatch(action, &params), Outcome::Failed { .. }),
            "{action} {params}"
        );
    }
    shell.shutdown();
}

#[test]
fn a5_a_pointer_at_each_entity_centre_hovers_an_entity_no_larger_that_contains_it() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    done(&mut shell, "view.zoom_to", json!({"percent": 400}));
    let entities = shell.canvas().unwrap().page().entities.clone();
    assert!(entities.len() > 20, "folio 103 has entities: {}", entities.len());
    let area = |r: &ew_viewmodel::ew_api::output::ew_dl::ew_units::Rect| {
        i128::from(r.max().x.0 - r.min().x.0) * i128::from(r.max().y.0 - r.min().y.0)
    };
    for (entity, r) in &entities {
        let (x, y) = rect_centre(*r);
        done(
            &mut shell,
            "canvas.pointer",
            json!({"x_mm": mm(x), "y_mm": mm(y)}),
        );
        let (got, hr) = shell
            .canvas()
            .unwrap()
            .hover()
            .unwrap_or_else(|| panic!("nothing hovered at the centre of {entity:?}"));
        // The hovered rectangle contains the point (the 4-point slop aside: inside the rectangle itself) and is no
        // larger than the entity's own.
        let inside = hr.min().x.0 <= x && x <= hr.max().x.0 && hr.min().y.0 <= y && y <= hr.max().y.0;
        assert!(inside, "{got:?} {hr:?} does not contain the centre of {entity:?}");
        assert!(area(&hr) <= area(r), "{got:?} is larger than {entity:?}");
    }
    done(&mut shell, "canvas.pointer", json!({}));
    assert_eq!(shell.canvas().unwrap().hover(), None);
    shell.shutdown();
}

#[test]
fn a8_the_transcript_and_the_describe_text_carry_the_canvas() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    done(&mut shell, "view.zoom_to", json!({"percent": 156.25}));
    let s = state(&shell);
    let c = &s["canvas"];
    assert_eq!(c["folio"], "103");
    assert_eq!(c["zoom"], 156.3);
    assert_eq!(c["grid"], json!([7_937_500, 3]));
    assert_eq!(c["pointer_nm"], Value::Null);
    assert_eq!(c["hover"], Value::Null);
    assert_eq!(c["backend"], "CPU");
    assert_eq!(c["statistics"], false);
    assert_eq!(c["renderer"], "auto");
    let view = c["view_nm"].as_array().unwrap();
    assert_eq!(view.len(), 4);
    // 1200 points at 156.25 % is 203.2 mm wide, centred on the sheet.
    assert_eq!(view[0], 148_500_000 - 101_600_000);
    assert_eq!(view[2], 148_500_000 + 101_600_000);
    // The members in the documented order.
    let text = script::state_json(&shell);
    let canvas = &text[text.find("\"canvas\"").unwrap()..];
    let order = [
        "\"folio\"",
        "\"zoom\"",
        "\"view_nm\"",
        "\"grid\"",
        "\"pointer_nm\"",
        "\"hover\"",
        "\"backend\"",
        "\"statistics\"",
        "\"renderer\"",
    ];
    let at: Vec<usize> = order.iter().map(|k| canvas.find(k).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{canvas}");
    // The describe lines, after the page-view line.
    let describe = shell.describe();
    let lines: Vec<&str> = describe.lines().collect();
    let pv = lines.iter().position(|l| *l == "page view: 103").unwrap();
    assert_eq!(
        lines[pv + 1],
        "canvas zoom 156.3 % view 46.900 37.267 250.100 172.733 mm"
    );
    assert_eq!(lines[pv + 2], "canvas grid on \"QET 10 px\"");
    assert_eq!(lines[pv + 3], "canvas pointer none");
    assert_eq!(lines[pv + 4], "canvas hover none");
    assert_eq!(lines[pv + 5], "canvas cursor none");
    assert_eq!(lines[pv + 6], "canvas link none");
    assert_eq!(lines[pv + 7], "canvas backend \"CPU\"");
    assert_eq!(lines[pv + 8], "canvas renderer auto");
    // The tool lines (WP-S2.03 part 2).
    assert_eq!(
        lines[pv + 9],
        "canvas tool \"Select\" state idle hint \"Click to select; drag for a rubber band\""
    );
    assert_eq!(lines[pv + 10], "canvas snap none");
    assert_eq!(lines[pv + 11], "canvas selection none");
    assert_eq!(lines[pv + 12], "canvas preview none");
    assert_eq!(lines[pv + 13], "canvas overlay 0");
    // WP-S2.17 part 2: the HUD over the canvas.
    assert_eq!(
        lines[pv + 14],
        "hud tool tool.select \"Select\" prompt \"Click to select; drag for a rubber band\""
    );
    assert!(
        lines[pv + 15].starts_with("workspace "),
        "no statistics line while off"
    );
    // The pointer at an entity's centre: that entity (or a smaller one there) in hover, its box in the describe line.
    let (entity, r) = *shell.canvas().unwrap().page().entities.last().unwrap();
    let (x, y) = rect_centre(r);
    done(
        &mut shell,
        "canvas.pointer",
        json!({"x_mm": mm(x), "y_mm": mm(y)}),
    );
    let (hovered, hr) = shell.canvas().unwrap().hover().unwrap();
    let s = state(&shell);
    assert_eq!(s["canvas"]["hover"], hovered.0.to_string());
    assert_eq!(s["canvas"]["pointer_nm"], json!([x, y]));
    let describe = shell.describe();
    let hover_line = describe.lines().find(|l| l.starts_with("canvas hover ")).unwrap();
    // The entity as its alias `@n`, a value token, never its UUID (FT-422).
    let alias = shell.alias_number(&hovered.0.to_string()).unwrap();
    assert!(
        hover_line.starts_with(&format!("canvas hover @{alias} box ")),
        "{hover_line}"
    );
    assert!(!describe.contains(&hovered.0.to_string()));
    assert!(hover_line.ends_with(&format!("{} mm", ew_viewmodel::canvas::mm3(hr.max().y.0))));
    if hovered == entity {
        assert_eq!(hr, r);
    }
    assert!(describe.contains(&format!(
        "canvas pointer {} {} mm",
        ew_viewmodel::canvas::mm3(x),
        ew_viewmodel::canvas::mm3(y)
    )));
    // The status bar's cursor field: the cell (FT-345), then the millimetres.
    let status = shell.status_fields(false);
    assert!(
        status
            .iter()
            .any(|f| f.starts_with("103-") && f.contains(" · x ") && f.ends_with(" mm")),
        "{status:?}"
    );
    assert!(status.contains(&"156 %".to_owned()), "{status:?}");
    // Grid off, statistics on, another backend label.
    done(&mut shell, "view.grid", json!({}));
    done(&mut shell, "view.frame_stats", json!({}));
    shell.set_canvas_backend(CanvasBackend::Gpu("Vulkan".into()));
    let s = state(&shell);
    assert_eq!(s["canvas"]["grid"], Value::Null);
    assert_eq!(s["canvas"]["statistics"], true);
    assert_eq!(s["canvas"]["backend"], "GPU Vulkan");
    let describe = shell.describe();
    assert!(describe.contains("canvas grid off\n"));
    assert!(
        describe.contains("canvas backend \"GPU Vulkan\"\ncanvas statistics shown\ncanvas renderer auto\n")
    );
    // View, Use the CPU renderer for the canvas: the setting, the transcript member, the describe line and the
    // checked menu item; the backend label is the host's (it applies the setting in its next frame).
    done(&mut shell, "view.cpu_canvas", json!({}));
    assert!(shell.cpu_canvas_setting());
    assert_eq!(shell.settings().canvas_backend, "cpu");
    assert_eq!(state(&shell)["canvas"]["renderer"], "cpu");
    assert!(shell.describe().contains("canvas renderer cpu\n"));
    assert!(checked(&shell, "view.cpu_canvas"));
    done(&mut shell, "view.cpu_canvas", json!({}));
    assert!(!shell.cpu_canvas_setting() && !checked(&shell, "view.cpu_canvas"));
    assert!(shell.status_fields(false).contains(&"Grid off".to_owned()));
    assert!(shell.status_fields(false).contains(&"GPU Vulkan".to_owned()));
    // No page: the member is null.
    done(&mut shell, "file.close", json!({}));
    assert_eq!(state(&shell)["canvas"], Value::Null);
    shell.shutdown();
}

#[test]
fn a8_the_canvas_steps_run_from_a_script_and_carry_no_time() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    let text = [
        r#"{"$schema":"ew.uiscript/1","label":"canvas"}"#,
        r#"{"action":"view.canvas_resize","params":{"width":1200,"height":800,"ppp":1.0}}"#,
        r#"{"action":"view.zoom_to","params":{"percent":156.25}}"#,
        r#"{"action":"canvas.pointer","params":{"x_mm":120.0,"y_mm":90.0}}"#,
        r#"{"action":"view.pan","params":{"dx":-40,"dy":12}}"#,
        r#"{"action":"view.zoom_in"}"#,
        r#"{"action":"canvas.pointer"}"#,
        r#"{"action":"ui.describe"}"#,
    ]
    .join("\n");
    let steps = script::parse(&text).unwrap();
    let mut out = Vec::new();
    let end = script::run(&mut shell, &mut NoExtension, &steps, false, &mut |l| {
        out.push(l.to_owned());
        Ok(())
    })
    .unwrap();
    assert_eq!(end.exit_code(), 0, "{out:?}");
    let last: Value = serde_json::from_str(out.last().unwrap()).unwrap();
    assert_eq!(
        last["state"]["canvas"]["zoom"], 200.0,
        "the ladder step above 156.25 %"
    );
    assert!(
        last["describe"]
            .as_str()
            .unwrap()
            .contains("canvas zoom 200.0 % view ")
    );
    // `canvas.snapshot` needs the renderer of the application: the core runner fails it.
    assert!(matches!(
        shell.dispatch("canvas.snapshot", &json!({"path": "x.png"})),
        Outcome::Failed { .. }
    ));
    shell.shutdown();
}

#[test]
fn a_pane_beyond_the_bounds_is_refused_and_the_view_stays() {
    use ew_viewmodel::canvas::{MAX_PANE_PIXELS, MAX_PANE_POINTS, MAX_PPP, bounded_pane};
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 1200, "height": 800, "ppp": 1.0}),
    );
    let bad = |shell: &mut ShellModel, params: Value, name: &str| match shell
        .dispatch("view.canvas_resize", &params)
    {
        Outcome::Failed { message } => assert!(message.contains(name), "{params}: {message}"),
        other => panic!("{params}: {other:?}"),
    };
    bad(
        &mut shell,
        json!({"width": MAX_PANE_POINTS + 1.0, "height": 10, "ppp": 0.25}),
        "width",
    );
    bad(
        &mut shell,
        json!({"width": 10, "height": MAX_PANE_POINTS + 1.0, "ppp": 0.25}),
        "height",
    );
    bad(
        &mut shell,
        json!({"width": 100, "height": 100, "ppp": MAX_PPP + 0.5}),
        "ppp",
    );
    // Within the points but beyond the pixels: 6000 points at ppp 2 is 12 000 pixels.
    bad(
        &mut shell,
        json!({"width": 6000, "height": 100, "ppp": 2.0}),
        "width",
    );
    bad(
        &mut shell,
        json!({"width": 1e300, "height": 1e300, "ppp": 1e300}),
        "ppp",
    );
    assert_eq!(shell.canvas_pane(), ((1200.0, 800.0), 1.0), "nothing was taken");
    assert_eq!(shell.canvas().unwrap().pane(), (1200.0, 800.0));
    // The largest pane is taken; beyond 4096 pixels the snapshot stitches its regions.
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": MAX_PANE_PIXELS / 2.0, "height": 600, "ppp": 2.0}),
    );
    assert_eq!(
        bounded_pane(20_000.0, 500.0, 9.0),
        (MAX_PANE_PIXELS / MAX_PPP, 500.0, MAX_PPP)
    );
    assert_eq!(bounded_pane(1200.0, 800.0, 1.0), (1200.0, 800.0, 1.0));
    shell.shutdown();
}

#[test]
fn the_bounded_pane_is_within_the_bounds_for_fractional_pixels_per_point() {
    use ew_viewmodel::canvas::{MAX_PANE_PIXELS, bounded_pane, pane_out_of_bounds};
    for ppp in [
        1.0, 1.1, 1.25, 1.5, 1.6, 1.75, 2.0, 2.2, 2.5, 3.3, 4.0, 7.9, 8.0, 9.0,
    ] {
        let (width, height, ppp) = bounded_pane(100_000.0, 100_000.0, ppp);
        assert_eq!(
            pane_out_of_bounds(width, height, ppp),
            None,
            "the host's own request at ppp {ppp} ({width} × {height} points) must be accepted"
        );
        assert!(width == width.floor(), "whole points at ppp {ppp}: {width}");
        assert!(
            width * ppp <= MAX_PANE_PIXELS + 0.5,
            "ppp {ppp}: {} pixels",
            width * ppp
        );
    }
    // A pane one whole pixel over the bound is still refused at a fractional ppp.
    assert_eq!(
        pane_out_of_bounds((MAX_PANE_PIXELS + 1.0) / 1.25, 10.0, 1.25),
        Some("width")
    );
}

#[test]
fn the_canvas_view_actions_leave_the_banner_standing() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    for (action, params) in [
        ("view.zoom_in", json!({})),
        ("view.zoom_out", json!({})),
        ("view.zoom_actual", json!({})),
        ("view.fit_page", json!({})),
        ("view.pan_left", json!({})),
        ("view.pan_right", json!({})),
        ("view.pan_up", json!({})),
        ("view.pan_down", json!({})),
        ("view.grid", json!({})),
        ("view.frame_stats", json!({})),
        ("view.zoom_to", json!({"percent": 200})),
        ("view.pan", json!({"dx": 5, "dy": 5})),
        ("view.canvas_resize", json!({"width": 900, "height": 600})),
        ("canvas.pointer", json!({"x_mm": 100, "y_mm": 100})),
    ] {
        shell.show_banner("Something to read");
        done(&mut shell, action, params);
        assert_eq!(
            shell.banner(),
            Some("Something to read"),
            "{action} cleared the banner"
        );
    }
    // Moving to another page is not a view of this page: it clears it, as before.
    done(&mut shell, "page.next", json!({}));
    assert_eq!(shell.banner(), None);
    shell.shutdown();
}

#[test]
fn a9_view_steps_fetch_no_page_and_re_execute_no_query() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    let calls = shell.page_content_calls();
    let (_, re_before) = shell.memo_totals().expect("the session has a memo store");
    let entities = shell.canvas().unwrap().page().entities.clone();
    for i in 0..200usize {
        let (action, params) = match i % 5 {
            0 => ("view.pan", json!({"dx": 13.5, "dy": -7})),
            1 => (
                "view.zoom_to",
                json!({"percent": 50 + (i % 7) * 40, "x_mm": 120, "y_mm": 80}),
            ),
            2 => {
                let (x, y) = rect_centre(entities[i % entities.len()].1);
                ("canvas.pointer", json!({"x_mm": mm(x), "y_mm": mm(y)}))
            }
            3 => (
                "view.canvas_resize",
                json!({"width": 900 + i, "height": 600, "ppp": if i % 2 == 0 { 1.0 } else { 1.25 }}),
            ),
            _ => ("view.zoom_in", json!({})),
        };
        let outcome = shell.dispatch(action, &params);
        assert!(
            matches!(outcome, Outcome::Done | Outcome::Refused { .. }),
            "{action}: {outcome:?}"
        );
        // What a frame reads: the status bar and the transcript state.
        let _ = shell.status_fields(false);
        let _ = script::state_json(&shell);
    }
    let (_, re_after) = shell.memo_totals().unwrap();
    assert_eq!(
        re_after - re_before,
        0,
        "pan, zoom, pointer and resize re-executed queries"
    );
    assert_eq!(shell.page_content_calls(), calls, "no further page_content call");
    shell.shutdown();
}

/// Revisions restart with every project session: a direct switch to another project at the same revision and
/// folio is a new canvas page, never the old project's (the fix round of WP-S2.02 part 2).
#[test]
fn a_project_switch_at_the_same_revision_and_folio_is_a_new_page() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell_on_103();
    let a = std::sync::Arc::clone(shell.canvas().unwrap().page());
    let rev = shell.revision();
    let other = tmp.0.join("other.ew");
    copy_dir(Path::new(GOLDEN_A), &other);
    done(
        &mut shell,
        "file.open",
        json!({"dir": other.to_string_lossy(), "read_only": true}),
    );
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    assert_eq!(shell.revision(), rev, "the same revision in the new session");
    let b = std::sync::Arc::clone(shell.canvas().unwrap().page());
    assert_eq!((b.rev, b.folio.as_str()), (a.rev, a.folio.as_str()));
    assert_ne!(b.session, a.session);
    assert!(
        !std::sync::Arc::ptr_eq(&a.list, &b.list),
        "the new project's content"
    );
    assert_eq!(Some(b.session), shell.project_epoch());
    shell.shutdown();
}
