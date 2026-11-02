// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 4 in the window (§12.4): Escape closes every dialog without its action (CP1); the toasts stand at
//! the canvas's top right, 300 px wide, newest on top (CP2, CP7); the palette lists its results with breadcrumbs and
//! key chips (CP4); the empty states are named groups and a finished job carries its outcome badge (CP5, FT-510); a
//! stale card shows its "Stale" badge as a node, never a "[stale]" text (CP8). Each followed by the accessible-name
//! walker.

use crate::support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// Waits for the running jobs without running frames, then settles. Polling with frames ran the window's clock on for
/// as long as the export took, so on a loaded machine the edit toast's 8 seconds ran out before the assertions
/// (FT-524: `cp2_cp7` saw one toast). The window's own path (the job wakes it, a frame takes the end) is `jobs.rs`'s.
fn run_until_jobs_end(h: &mut Harness<'_, App>) {
    h.state_mut().shell_mut().wait_jobs();
    settle(h);
    assert_eq!(h.state().shell().jobs().running(), 0);
}

fn group_named(h: &Harness<'_, App>, name: &str) -> bool {
    h.get_all_by_role(egui::Role::Group)
        .any(|n| n.accesskit_node().label().as_deref() == Some(name))
}

fn has_text(h: &Harness<'_, App>, text: &str) -> bool {
    h.root().children_recursive().any(|n| {
        let n = n.accesskit_node();
        n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text)
    })
}

#[test]
fn cp1_escape_closes_every_dialog_without_its_action() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    let rev = h.state().shell().revision();
    for (action, title) in [
        ("help.about", None),
        ("help.shortcuts", Some("Keyboard shortcuts")),
        ("page.goto", Some("Go to page or cell")),
        ("page.new", Some("New page")),
        ("devices.renumber", Some("Renumber devices")),
        ("project.purge_assets", None),
    ] {
        let _ = h.state_mut().shell_mut().dispatch(action, &json!({}));
        settle(&mut h);
        if h.state().shell().dialog().is_none() {
            // Purge with nothing unused says so in a toast instead of a dialog.
            continue;
        }
        if let Some(title) = title {
            assert!(
                h.query_all_by_label(title).next().is_some(),
                "{action}: the title {title}"
            );
        }
        h.key_press(Key::Escape);
        settle(&mut h);
        assert!(h.state().shell().dialog().is_none(), "{action}: Escape closes it");
        assert_eq!(h.state().shell().revision(), rev, "{action}: nothing changed");
    }
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn cp2_cp7_the_toasts_stand_at_the_canvas_top_right_newest_on_top() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    dispatch(
        &mut h,
        "file.export_pdf",
        json!({ "path": tmp.0.join("a.pdf").to_string_lossy() }),
    );
    run_until_jobs_end(&mut h);
    let toasts: Vec<egui::Rect> = h
        .get_all_by_role(egui::Role::Group)
        .filter(|n| n.accesskit_node().label().as_deref() == Some("Notification"))
        .map(|n| n.rect())
        .collect();
    assert_eq!(toasts.len(), 2, "two keys, two toasts (at most three)");
    let edit = h.get_by_label("Set function text on 2 devices").rect();
    let export = h.get_by_label_contains("Exported a.pdf").rect();
    assert!(export.top() < edit.top(), "the newest on top");
    let pane = h.state().command_surface_rects().2.expect("the canvas pane");
    for r in &toasts {
        assert!((r.width() - 300.0).abs() <= 2.0, "300 px wide: {r:?}");
        assert!(
            (pane.right() - 12.0 - r.right()).abs() <= 2.0,
            "12 px from the canvas's right edge: {r:?} in {pane:?}"
        );
        assert!(
            r.top() >= pane.top() + 12.0 - 2.0,
            "under the canvas's top: {r:?}"
        );
    }
    let (upper, lower) = if toasts[0].top() < toasts[1].top() {
        (toasts[0], toasts[1])
    } else {
        (toasts[1], toasts[0])
    };
    assert!(
        (lower.top() - upper.bottom() - 8.0).abs() <= 2.0,
        "space.3 between toasts: {upper:?} {lower:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Doc 16a §6.12: with the quit prompt open the toast is not a second focus: its layer lies below the modal's
/// (the backdrop dims it); without a dialog it stands in the foreground.
#[test]
fn a_modal_dialog_puts_the_toasts_below_its_backdrop() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    h.run_ok();
    let toasts = egui::Id::unique("ew-toasts");
    let layer_of = |h: &Harness<'_, App>| {
        h.ctx
            .memory(|m| m.layer_ids().filter(|l| l.id == toasts).last())
            .expect("the toasts' layer")
    };
    assert_eq!(layer_of(&h).order, egui::Order::Foreground);
    let _ = h.state_mut().shell_mut().dispatch("file.close", &json!({}));
    h.run_ok();
    h.run_ok();
    let modal = h
        .ctx
        .memory(|m| m.top_modal_layer())
        .expect("the quit prompt is modal");
    let toast = egui::LayerId::new(egui::Order::Middle, toasts);
    assert!(
        h.ctx.memory(|m| m.areas().is_visible(&toast)),
        "the toast is shown in the middle order"
    );
    let order: Vec<egui::LayerId> = h.ctx.memory(|m| m.layer_ids().collect());
    let at = |l: egui::LayerId| order.iter().position(|o| *o == l);
    assert!(
        at(toast) < at(modal) && !h.ctx.memory(|m| m.is_above_modal_layer(toast)),
        "the toast {toast:?} lies below the modal {modal:?}: {order:?}"
    );
}

#[test]
fn cp4_the_palette_rows_carry_breadcrumbs_and_key_chips() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "tools.palette", json!({}));
    settle(&mut h);
    let entries = h.state().shell().palette_entries();
    assert!(!entries.is_empty());
    assert!(entries.iter().all(|e| !e.breadcrumb.is_empty()));
    // The field has the keyboard focus: type into it.
    h.event(egui::Event::Text("save as".into()));
    settle(&mut h);
    let entries = h.state().shell().palette_entries();
    let save_as = entries.iter().find(|e| e.id == "file.save_as").expect("Save as…");
    assert_eq!(save_as.breadcrumb, "File");
    assert!(
        h.query_by_label_contains("Save as…").is_some(),
        "the row is named by its label"
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn cp5_ft510_empty_states_and_the_jobs_outcome_badge() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "jobs.show", json!({}));
    settle(&mut h);
    assert!(group_named(&h, "No jobs yet"), "the Jobs pane's empty state");
    dispatch(
        &mut h,
        "file.export_pdf",
        json!({ "path": tmp.0.join("a.pdf").to_string_lossy() }),
    );
    run_until_jobs_end(&mut h);
    assert!(!group_named(&h, "No jobs yet"));
    assert!(h.query_by_label("Done").is_some(), "FT-510: the outcome badge");
    dispatch(&mut h, "preview.show", json!({}));
    settle(&mut h);
    assert!(
        group_named(&h, "No preview is open"),
        "the Preview tab's empty state"
    );
    assert!(
        has_text(
            &h,
            "Renumbering, a quick fix or an import shows its changes here before you apply them."
        ),
        "its sentence"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn cp8_a_stale_card_shows_its_badge_and_apply_stays_the_primary() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_some());
    assert!(h.query_by_label("Stale").is_none(), "fresh: no badge");
    // An edit elsewhere makes the card stale.
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
            "path": "function_text", "value": "x"}}),
    );
    settle(&mut h);
    assert!(
        h.state().shell().properties().card().is_some(),
        "the card stays open"
    );
    assert!(
        h.query_all_by_label("Stale").next().is_some(),
        "the Stale badge is a node"
    );
    assert!(
        !h.root()
            .children_recursive()
            .any(|n| n.accesskit_node().label().is_some_and(|l| l.contains("[stale]"))),
        "no [stale] text"
    );
    assert!(h.query_all_by_label("Apply").next().is_some());
    h.state_mut().shell_mut().shutdown();
}

/// FT-524: a toast's 8 seconds (WP-S2.09 D6) run on the window's frame clock (egui's `input.time`), which is the wall
/// clock in the window and the harness's steps here, so how long a loaded machine takes between two frames does not
/// decide whether a test still sees a toast. The toast stays while less than 8 seconds of frames have passed since
/// the frame that first painted it, and goes in the first frame at 8 seconds.
#[test]
fn ft_524_a_toast_lives_8_seconds_of_the_frame_clock() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    h.step();
    let shown = h.ctx.input(|i| i.time);
    assert_eq!(h.state().shell().notifications().visible().len(), 1);
    for _ in 0..1000 {
        h.step();
        let age = h.ctx.input(|i| i.time) - shown;
        let visible = h.state().shell().notifications().visible().len();
        if age < 8.0 {
            assert_eq!(visible, 1, "on screen at {age} s");
        } else {
            assert_eq!(visible, 0, "gone at {age} s");
            h.state_mut().shell_mut().shutdown();
            return;
        }
    }
    panic!("the frame clock did not reach 8 seconds");
}

/// A harness on Golden A with one toast (an edit's) painted in its first frame; returns the frame time of that frame.
fn one_toast(h: &mut Harness<'_, App>) -> f64 {
    settle(h);
    dispatch(h, "page.goto", json!({"folio": "101"}));
    dispatch(h, "select.set", json!({"refs": ["tag:-101F1", "tag:-102F1"]}));
    dispatch(
        h,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    h.step();
    assert_eq!(h.state().shell().notifications().visible().len(), 1);
    h.ctx.input(|i| i.time)
}

/// One frame at `t` seconds after `shown` on the frame clock; the opacity of the one toast, `None` once it is gone.
fn opacity_at(h: &mut Harness<'_, App>, shown: f64, t: f64) -> Option<f32> {
    h.input_mut().time = Some(shown + t);
    h.step();
    if h.state().shell().notifications().visible().is_empty() {
        return None;
    }
    let opacities = h.state().toast_opacities();
    assert!(opacities.len() <= 1, "{opacities:?}");
    opacities.first().map(|(_, a)| *a)
}

/// FT-1027, ID-1107: "a toast fades out as it fades in (`motion.base`), on the frame clock." The opacity is 0 in the
/// frame that first paints it and 1 at `motion.base`; 1 at the start of the fade out, `motion.base` before the 8 s,
/// half way through it, and the toast is gone at the end.
#[test]
fn ft1027_a_toast_fades_in_and_out_over_motion_base_on_the_frame_clock() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    let shown = one_toast(&mut h);
    let fade = f64::from(ew_ui::theme::current(&h.ctx).motion_base) / 1000.0;
    assert_eq!(fade, 0.14, "motion.base");
    assert_eq!(h.state().toast_opacities()[0].1, 0.0, "the first frame");
    let near = |a: Option<f32>, want: f32, what: &str| {
        let a = a.unwrap_or_else(|| panic!("{what}: the toast is gone"));
        assert!((a - want).abs() < 1e-3, "{what}: {a}, want {want}");
    };
    near(opacity_at(&mut h, shown, fade / 2.0), 0.5, "half way in");
    near(opacity_at(&mut h, shown, fade), 1.0, "faded in");
    near(opacity_at(&mut h, shown, 4.0), 1.0, "standing");
    near(
        opacity_at(&mut h, shown, 8.0 - fade),
        1.0,
        "the start of the fade out",
    );
    near(opacity_at(&mut h, shown, 8.0 - fade / 2.0), 0.5, "half way out");
    near(
        opacity_at(&mut h, shown, 8.0 - 0.001),
        0.007,
        "the end of the fade out",
    );
    assert_eq!(opacity_at(&mut h, shown, 8.0), None, "gone at 8 s");
    assert_eq!(h.state().shell().notifications().visible().len(), 0);
    h.state_mut().shell_mut().shutdown();
}

/// FT-1027, ID-1107: "Hovering still pauses it." A hovered toast's clock stands still: it is on screen, unfaded, 20 s
/// after it came; once the pointer leaves, its 8 s go on from where they stood.
#[test]
fn ft1027_hovering_a_toast_pauses_its_fade() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    let shown = one_toast(&mut h);
    assert_eq!(opacity_at(&mut h, shown, 1.0), Some(1.0));
    let toast = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Notification"))
        .expect("the toast's node")
        .rect()
        .center();
    h.input_mut().events.push(egui::Event::PointerMoved(toast));
    for t in [2.0, 7.95, 20.0] {
        assert_eq!(opacity_at(&mut h, shown, t), Some(1.0), "hovered at {t} s");
    }
    // The pointer leaves: 2 s of its life had passed (the frame clock counts from the frame after the pointer came to
    // the frame after it left), the rest runs on.
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(egui::pos2(1.0, 400.0)));
    assert_eq!(opacity_at(&mut h, shown, 25.0), Some(1.0), "2 s of its life");
    assert_eq!(opacity_at(&mut h, shown, 30.0), Some(1.0), "7 s of its life");
    assert_eq!(opacity_at(&mut h, shown, 31.0), None, "8 s of its life are over");
    h.state_mut().shell_mut().shutdown();
}

/// C46-p1#5 (ID-1139 as drawn): at the 1280 × 800 floor, a click on the greyed Symbol button of a panel layout page
/// shows a toast whose title stands whole on two rows (its text runs as drawn), never cut with an ellipsis.
#[test]
fn a_greyed_commands_toast_title_stands_whole_on_two_rows_at_the_floor() {
    const TITLE: &str = "A panel layout page takes no symbols or wires";
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let shell =
        ew_ui::ew_viewmodel::shell::ShellModel::new(ew_ui::ew_viewmodel::shell::Mode::Gui, tmp.config());
    let app = App::new(shell, Box::new(Scripted::default()), Some(dir));
    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .with_context_setup(ew_ui::app::setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    settle(&mut h);
    support::wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "902"}));
    settle(&mut h);
    h.get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Symbol"))
        .expect("the Symbol button")
        .click();
    settle(&mut h);
    // The title's label carries the toast's whole text as its live value; its text runs are the rows as drawn.
    let whole = format!("{TITLE}: draw on a schematic page");
    let label = h
        .get_all_by_role(egui::Role::Label)
        .find(|n| n.accesskit_node().value().as_deref() == Some(whole.as_str()))
        .expect("the toast's title");
    let rows: Vec<String> = label
        .children()
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::TextRun)
        .map(|n| n.accesskit_node().value().unwrap_or_default())
        .collect();
    assert_eq!(rows.len(), 2, "two rows: {rows:?}");
    assert_eq!(rows.concat().trim_end(), TITLE, "whole, uncut: {rows:?}");
    h.state_mut().shell_mut().shutdown();
}
