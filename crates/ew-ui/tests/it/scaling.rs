// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 5 in the window (D14; AX-2 to AX-4; FR-UI-027, NFR-A11Y-04; rulings 4 to 7): View ▸ UI scale
//! and the first run offer 250 % and 300 % and `settings.toml` keeps them; the in-window core dialogs (New project,
//! Renumber devices…, Renumber pages…, the print pre-dialog, the find and replace pane, the settings switches, the
//! first-run steps, the recovery dialog, the parts import card) pass `assert_accessible` and `assert_fits` at 300 % on a
//! 1280 × 720-point window (a 3840 × 2160 display), and at 100 % with the minimum text size 14 pt, which raises the
//! small text tokens to 14 pt; with reduced motion egui's animation time is zero and a Ctrl+wheel step zooms in its
//! own frame, and the CPU canvas turns it on while the setting is unset. Open and Export PDF's file choosers are the
//! system's own dialogs, which the window does not draw.

use crate::support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::{Modifiers, MouseWheelSource, MouseWheelUnit, TouchPhase, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::{App, Pick};
use ew_ui::ew_viewmodel::settings::Settings;
use ew_ui::ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use ew_ui::ew_viewmodel::{key, strings};
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, harness_with, settle, wait_for_page};

/// D14's window, in points.
const WINDOW: Vec2 = Vec2::new(1280.0, 720.0);

/// How a dialog is shown: at 300 % (ppp 3.0), or at 100 % with the minimum text size 14 pt.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Size {
    At300,
    Min14,
}

/// The UI scale of `size` on a display of the window's points at that scale and, at 100 %, the minimum text size
/// 14 pt.
#[track_caller]
fn apply(h: &mut Harness<'_, App>, size: Size) {
    let scale = if size == Size::At300 { 3.0 } else { 1.0 };
    dispatch(h, "view.ui_scale", json!({ "scale": scale }));
    if size == Size::Min14 {
        dispatch(h, "view.min_font", json!({ "pt": 14 }));
    }
    h.set_size(WINDOW * scale);
    settle(h);
    assert!((h.ctx.pixels_per_point() - scale).abs() < 1e-6);
    let content = h.ctx.content_rect().size();
    assert!((content - WINDOW).length() < 0.5, "{content:?}");
    if size == Size::Min14 {
        let t = ew_ui::theme::current(&h.ctx);
        for (name, token) in [
            ("caption", &t.type_caption),
            ("small", &t.type_small),
            ("label", &t.type_label),
            ("body", &t.type_body),
            ("mono", &t.type_mono),
        ] {
            assert_eq!(token.size, 14.0, "type.{name}");
        }
        assert_eq!((t.type_title.size, t.type_display.size), (15.0, 20.0));
    }
}

/// One step of a dialog: the frame settles, every input is named and every node fits, and an open dialog's buttons lie
/// inside the window down as well.
#[track_caller]
fn step(h: &mut Harness<'_, App>) {
    settle(h);
    support::assert_fits(h);
    assert_eq!(support::footer_problems(h), Vec::<String>::new());
}

/// Golden A open in the window (folio 101), at `size`.
fn on_golden(tmp: &Tmp, size: Size) -> Harness<'static, App> {
    let mut h = harness(tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    settle(&mut h);
    wait_for_page(&mut h);
    apply(&mut h, size);
    h
}

/// New project: the start page's New project, the folder the system's chooser answers, the new project's page.
fn new_project(size: Size) {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    platform
        .answers
        .borrow_mut()
        .push_back(Pick::Path(tmp.0.join("first.ew")));
    let shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    let mut h = harness_with(shell, platform, None);
    settle(&mut h);
    apply(&mut h, size);
    step(&mut h);
    h.get_by_label(strings::text(key!("start-new"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().project_name(), Some("first.ew"));
    wait_for_page(&mut h);
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// Renumber devices… (with a numbering scheme, as `dutch.rs` sets it) and Renumber pages….
fn renumber(size: Size) {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, size);
    support::page_based_numbering(&mut h);
    for action in ["devices.renumber", "pages.renumber"] {
        dispatch(&mut h, action, json!({}));
        step(&mut h);
        assert!(
            matches!(h.state().shell().dialog(), Some(Dialog::Form(_))),
            "{action}"
        );
        dispatch(&mut h, "ui.dismiss", json!({}));
        step(&mut h);
    }
    h.state_mut().shell_mut().shutdown();
}

/// The print pre-dialog: Export PDF's choices (the range, the paper, the preview).
fn print(size: Size) {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, size);
    dispatch(&mut h, "file.print", json!({"range": "current"}));
    for _ in 0..3000 {
        h.run_ok();
        if h.state().shell().print_set().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(h.state().shell().print_set().is_some(), "the print set was built");
    step(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Print(_))));
    h.state_mut().shell_mut().shutdown();
}

/// The find and replace pane, with a search's results.
fn find_and_replace(size: Size) {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, size);
    dispatch(&mut h, "find.open_replace", json!({}));
    step(&mut h);
    assert!(h.state().shell().find_shown());
    dispatch(&mut h, "find.text", json!({"text": "-102"}));
    dispatch(&mut h, "find.run", json!({}));
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The settings switches: the start page's language and keymap quick switches, View ▸ UI scale and View ▸ Minimum
/// text size.
fn settings_switches(size: Size) {
    let tmp = Tmp::new();
    let shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    apply(&mut h, size);
    step(&mut h);
    h.get_by_label(strings::text(key!("keymap-kicad"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().settings().keymap, "kicad");
    h.get_by_label(strings::text(key!("keymap-ew"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("language-nl"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("language-en"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().settings().language, "en");
    for submenu in [key!("submenu-ui-scale"), key!("submenu-min-font")] {
        support::open_menu(
            &mut h,
            &[strings::text(key!("menu-view")), strings::text(submenu)],
        );
        support::assert_accessible(&h);
        support::assert_fits(&h);
        h.key_press(egui::Key::Escape);
        h.key_press(egui::Key::Escape);
        step(&mut h);
    }
    h.state_mut().shell_mut().shutdown();
}

/// The first-run steps, each shown, with the appearance step's scales.
fn first_run(size: Size) {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    shell.start_first_run_if_needed();
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    apply(&mut h, size);
    step(&mut h);
    for _ in 0..4 {
        h.get_by_label(strings::text(key!("firstrun-next"))).click();
        step(&mut h);
    }
    h.get_by_label(strings::text(key!("firstrun-start-none"))).click();
    step(&mut h);
    assert!(h.state().shell().first_run().is_none());
    h.state_mut().shell_mut().shutdown();
}

/// The recovery dialog of a session that ended without a clean close (`jobs.rs` JB7).
fn recovery(size: Size) {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    {
        let mut crashed = ShellModel::new(Mode::Gui, tmp.config());
        let opened = crashed.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() }));
        assert_eq!(opened, Outcome::Done);
        let made = crashed.dispatch(
            "command.run",
            &json!({"cmd": "page.create", "params": {"number": 996}}),
        );
        assert_eq!(made, Outcome::Done);
        crashed.shutdown();
    }
    // The scale first: an open dialog refuses the settings.
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    apply(&mut h, size);
    dispatch(&mut h, "file.open", json!({ "dir": dir.to_string_lossy() }));
    step(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::Recovery { .. })
    ));
    h.state_mut().shell_mut().shutdown();
}

fn golden_parts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv")
}

/// File, Import, Parts table…: the column mapping step, then the import card (`parts_import.rs`).
fn parts_import(size: Size) {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let params = json!({"dir": tmp.0.join("proj.ew").to_string_lossy(), "template": "builtin"});
    assert_eq!(shell.dispatch("file.new", &params), Outcome::Done);
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    wait_for_page(&mut h);
    apply(&mut h, size);
    let path = golden_parts();
    dispatch(&mut h, "parts.import", json!({"path": path.to_string_lossy()}));
    step(&mut h);
    dispatch(&mut h, "parts.map_import", json!({}));
    step(&mut h);
    assert!(h.state().shell().properties().card().is_some());
    h.state_mut().shell_mut().shutdown();
}

/// ID-1278 ruling 13: the check down bites. Renumber pages… fits the 1280 × 720-point window at 300 % with its Primary
/// and Cancel; in a window too short for it, its buttons are reported as leaving the window.
#[test]
fn ax3_a_footer_below_the_window_is_reported() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, Size::At300);
    dispatch(&mut h, "pages.renumber", json!({}));
    step(&mut h);
    h.set_size(Vec2::new(WINDOW.x * 3.0, 60.0));
    settle(&mut h);
    let problems = support::footer_problems(&h);
    let spec = ew_ui::ew_viewmodel::dialogs::spec(h.state().shell().dialog().unwrap(), h.state().shell());
    for button in spec.buttons() {
        let quoted = format!("{:?}", button.label);
        assert!(
            problems.iter().any(|p| p.starts_with(&quoted)),
            "{quoted} in {problems:?}"
        );
    }
    h.state_mut().shell_mut().shutdown();
}

/// The first run's steps are checked too. Its first step fits at 300 %; in a window too short for it, its
/// Skip and Next are reported as leaving the window.
#[test]
fn ax3_a_first_run_footer_below_the_window_is_reported() {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    shell.start_first_run_if_needed();
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    apply(&mut h, Size::At300);
    step(&mut h);
    h.set_size(Vec2::new(WINDOW.x * 3.0, 60.0));
    settle(&mut h);
    let problems = support::footer_problems(&h);
    for key in [key!("firstrun-skip"), key!("firstrun-next")] {
        let quoted = format!("{:?}", strings::text(key));
        assert!(
            problems.iter().any(|p| p.starts_with(&quoted)),
            "{quoted} in {problems:?}"
        );
    }
    h.state_mut().shell_mut().shutdown();
}

/// Runs a flow at 300 % and with the minimum text size 14 pt; a failure's captured output names the size.
fn at_both_sizes(flow: fn(Size)) {
    for size in [Size::At300, Size::Min14] {
        eprintln!("at {size:?}");
        flow(size);
    }
}

#[test]
fn ax3_new_project_at_300_and_with_14_pt_text() {
    at_both_sizes(new_project);
}

#[test]
fn ax3_renumber_devices_and_pages_at_300_and_with_14_pt_text() {
    at_both_sizes(renumber);
}

#[test]
fn ax3_the_print_pre_dialog_at_300_and_with_14_pt_text() {
    at_both_sizes(print);
}

#[test]
fn ax3_find_and_replace_at_300_and_with_14_pt_text() {
    at_both_sizes(find_and_replace);
}

#[test]
fn ax3_the_settings_switches_at_300_and_with_14_pt_text() {
    at_both_sizes(settings_switches);
}

#[test]
fn ax3_the_first_run_steps_at_300_and_with_14_pt_text() {
    at_both_sizes(first_run);
}

#[test]
fn ax3_the_recovery_dialog_at_300_and_with_14_pt_text() {
    at_both_sizes(recovery);
}

#[test]
fn ax3_the_parts_import_card_at_300_and_with_14_pt_text() {
    at_both_sizes(parts_import);
}

/// AX-2: View ▸ UI scale and the first run's appearance step offer 250 % and 300 %; choosing either writes it to
/// `settings.toml`, which reads it back.
#[test]
fn ax2_the_menu_and_the_first_run_offer_250_and_300_percent() {
    let tmp = Tmp::new();
    let config = tmp.config().config_dir.expect("a settings directory");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    shell.start_first_run_if_needed();
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    for _ in 0..3 {
        h.get_by_label(strings::text(key!("firstrun-next"))).click();
        settle(&mut h);
    }
    assert!(h.query_by_label("300 %").is_some());
    h.get_by_label("250 %").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().ui_scale, 2.5);
    assert_eq!(Settings::load(&config).settings.ui_scale, 2.5);
    dispatch(&mut h, "first_run.finish", json!({"start": "none"}));
    dispatch(&mut h, "view.ui_scale", json!({ "scale": 1.0 }));
    settle(&mut h);
    support::open_menu(
        &mut h,
        &[
            strings::text(key!("menu-view")),
            strings::text(key!("submenu-ui-scale")),
        ],
    );
    assert!(support::off_chrome(&h, "250 %").is_some());
    support::menu_node(&h, "300 %").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().ui_scale, 3.0);
    assert_eq!(Settings::load(&config).settings.ui_scale, 3.0);
    h.state_mut().shell_mut().shutdown();
}

/// egui's animation time in the theme the window shows.
fn animation_time(h: &Harness<'_, App>) -> f32 {
    h.ctx.style_of(h.ctx.theme()).animation_time
}

/// The zoom of the canvas at 100 % with the pointer at its centre, and its zoom one frame after a Ctrl+wheel line up.
fn ctrl_wheel_one_frame(h: &mut Harness<'_, App>) -> (f64, f64) {
    dispatch(h, "view.zoom_to", json!({"percent": 100.0}));
    settle(h);
    let b = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let ppp = f64::from(h.ctx.pixels_per_point());
    let at = egui::Pos2::new(
        ((b.x0 + b.x1) / 2.0 / ppp) as f32,
        ((b.y0 + b.y1) / 2.0 / ppp) as f32,
    );
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.step();
    let before = h.state().shell().canvas().unwrap().zoom();
    let input = h.input_mut();
    input.events.push(egui::Event::ModifiersChanged(Modifiers::CTRL));
    input.events.push(egui::Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: Vec2::new(0.0, 1.0),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::CTRL,
    });
    h.step();
    let after = h.state().shell().canvas().unwrap().zoom();
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(Modifiers::NONE));
    settle(h);
    (before, after)
}

/// AX-4: with reduced motion egui's animation time is zero and a Ctrl+wheel line zooms to its whole factor (egui's
/// `exp(scroll_zoom_speed × line_scroll_speed)`) in its own frame; without it egui spreads the step over frames.
#[test]
fn ax4_reduced_motion_stops_animation_and_the_zoom_jumps() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    settle(&mut h);
    wait_for_page(&mut h);
    // The harness draws on the CPU, which reduces motion while the setting is unset: motion on.
    dispatch(&mut h, "view.reduce_motion", json!({"on": false}));
    settle(&mut h);
    assert!(!h.state().shell().reduce_motion());
    assert!(animation_time(&h) > 0.0);
    let (before, smoothed) = ctrl_wheel_one_frame(&mut h);
    let factor = h
        .ctx
        .options(|o| f64::from(o.input_options.scroll_zoom_speed * o.input_options.line_scroll_speed).exp());
    let target = before * factor;
    assert!(
        smoothed < target - 1e-3,
        "egui's smoothing: {smoothed} of {target}"
    );
    dispatch(&mut h, "view.reduce_motion", json!({}));
    settle(&mut h);
    assert!(h.state().shell().reduce_motion());
    assert_eq!(h.state().shell().settings().reduce_motion, Some(true));
    assert_eq!(animation_time(&h), 0.0);
    assert_eq!(ew_ui::theme::current(&h.ctx).motion_base, 0);
    let (before, jumped) = ctrl_wheel_one_frame(&mut h);
    assert!(
        (jumped - before * factor).abs() < 1e-6,
        "one frame to the target: {jumped} of {}",
        before * factor
    );
    h.state_mut().shell_mut().shutdown();
}

/// ID-1278 ruling 12 (D14, doc 16 §10.4, doc 16a §4.12): with the setting unset, a canvas that draws on the
/// GPU keeps motion, and a GPU start that fails (FR-UI-028's fallback to the CPU) leaves reduced motion on.
#[test]
fn ax4_a_failed_gpu_start_leaves_reduced_motion_on() {
    use ew_ui::gpu::{AdapterSummary, DeviceKind, GpuApi, GpuError};
    let vulkan = || AdapterSummary {
        kind: DeviceKind::Discrete,
        api: GpuApi::Vulkan,
        name: "test GPU".into(),
    };
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    settle(&mut h);
    wait_for_page(&mut h);
    assert!(h.state().shell().reduce_motion(), "unset, on the CPU canvas");
    h.state_mut().gpu_host_mut().simulate(vulkan(), Ok(()));
    settle(&mut h);
    // The appearance follows on the frame after the one whose switch rule chose the GPU.
    h.step();
    settle(&mut h);
    assert_eq!(h.state().shell().canvas_backend().label(), "GPU Vulkan");
    assert!(!h.state().shell().reduce_motion(), "unset, on the GPU");
    assert!(animation_time(&h) > 0.0);
    h.state_mut().shell_mut().shutdown();
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    settle(&mut h);
    wait_for_page(&mut h);
    h.state_mut()
        .gpu_host_mut()
        .simulate(vulkan(), Err(GpuError::Init("pipeline creation failed".into())));
    settle(&mut h);
    assert_eq!(h.state().shell().canvas_backend().label(), "CPU");
    assert_eq!(h.state().shell().settings().reduce_motion, None);
    assert!(h.state().shell().reduce_motion());
    assert_eq!(animation_time(&h), 0.0);
    h.state_mut().shell_mut().shutdown();
}

/// AX-4: with the setting unset, the CPU canvas (View ▸ Use the CPU renderer for the canvas) turns reduced motion
/// on; the setting, once set, decides.
#[test]
fn ax4_the_cpu_canvas_reduces_motion_while_the_setting_is_unset() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    dispatch(&mut h, "view.cpu_canvas", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().settings().reduce_motion, None);
    assert!(h.state().shell().reduce_motion());
    assert_eq!(animation_time(&h), 0.0);
    dispatch(&mut h, "view.reduce_motion", json!({"on": false}));
    settle(&mut h);
    assert!(!h.state().shell().reduce_motion());
    assert!(animation_time(&h) > 0.0);
    h.state_mut().shell_mut().shutdown();
}
