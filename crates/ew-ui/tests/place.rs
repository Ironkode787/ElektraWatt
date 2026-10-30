// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 1 in the window without rendering (acceptance checks PL6 and PL8): AC-28's terminal step on folio
//! 103 from the keyboard with the ghost's predicted tag after the window's dwell, the picker popup driven by keys,
//! and the exact `Session` counts of the place tool, the picker and a move in the window. The accessible-name walker
//! runs after every step.

mod support;

use egui::{Key, Modifiers, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": folio }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

fn press(h: &mut Harness<'_, App>, key: Key) {
    h.key_press(key);
    settle(h);
}

fn reps(h: &Harness<'_, App>) -> usize {
    h.state().shell().canvas().unwrap().page().tables.reps.len()
}

/// The window position (points) of the sheet point `p` on the focused canvas.
fn screen(h: &Harness<'_, App>, p: (i64, i64)) -> Pos2 {
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    Pos2::new(
        bounds.x0 as f32 + ((p.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((p.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    )
}

#[test]
fn pl6_a_terminal_joins_x1031_from_the_keyboard_with_the_predicted_tag_after_the_dwell() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    let before = reps(&h);
    // The terminal symbol armed for strip -X1031 (the picker's headless twin; the window's popup lists the same rows).
    dispatch(
        &mut h,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
    );
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "place");
    // The arrow keys move the grid cursor; the ghost follows it.
    press(&mut h, Key::ArrowRight);
    press(&mut h, Key::ArrowRight);
    let ghost = h.state().shell().tool_ghost().expect("a ghost");
    assert!(format!("{ghost:?}").contains("terminal"), "{ghost:?}");
    // The window's dwell timer previews the command after 80 ms of rest; the ghost then shows the predicted tag.
    // The rest is the action `canvas.dwell` (D11): dispatched as the window's dwell timer dispatches it, so the test
    // reads no clock and sleeps nowhere (fix round, item 6; ID-559).
    dispatch(&mut h, "canvas.dwell", json!({"ms": 80}));
    settle(&mut h);
    let Some(ew_ui::ew_viewmodel::tools::Ghost::Place { tag: Some(tag), .. }) =
        h.state().shell().tool_ghost()
    else {
        panic!("no predicted tag after the rest");
    };
    assert!(h.state().shell().session_calls().preview_with >= 1);
    assert!(
        h.state()
            .shell()
            .tool_overlay()
            .labels()
            .any(|l| l.contains(&tag)),
        "the ghost's label shows {tag}"
    );
    assert!(
        support::hud_value(&h).contains("-X1031:30"),
        "the HUD names the target (WP-S2.17 part 2)"
    );
    settle(&mut h);
    // Enter commits; the tool stays armed with the next designation; Ctrl+Z takes the terminal back.
    press(&mut h, Key::Enter);
    assert_eq!(reps(&h), before + 1);
    assert_eq!(h.state().shell().tool().name(), "place");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    assert_eq!(reps(&h), before);
    press(&mut h, Key::Escape);
    assert_eq!(h.state().shell().tool().name(), "select");
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_picker_popup_is_driven_by_its_keys_and_floats_a_starter_symbol() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    // I opens the picker on the canvas; the filter field has the focus.
    press(&mut h, Key::I);
    assert!(h.state().shell().picker().is_some(), "I opens the picker");
    h.get_all_by_role(egui::Role::TextInput)
        .next()
        .expect("the filter field")
        .type_text("contact-no@");
    settle(&mut h);
    assert_eq!(h.state().shell().picker().unwrap().filter, "contact-no@");
    assert_eq!(h.state().shell().picker().unwrap().rows.len(), 1);
    // Enter chooses the starter row: the symbol floats without a commit (FT-656: the placement embeds it, one undo
    // step); the letter comes from its category (K for a contact of the identifier set).
    let rev = h.state().shell().revision();
    press(&mut h, Key::Enter);
    let shell = h.state().shell();
    assert!(shell.picker().is_none());
    assert_eq!(shell.tool().name(), "place");
    assert_eq!(shell.revision(), rev, "choosing commits nothing");
    // Escape returns to Select without a change; the menu's Insert items are registry entries.
    press(&mut h, Key::Escape);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert_eq!(h.state().shell().revision(), rev);
    h.state_mut().shell_mut().shutdown();
}

/// Cycle 37 (window check W37): the picker opened by `tool.place` takes the keyboard from the first key; text typed in
/// the frame that first shows it reaches its filter field ("relay", not "elay").
#[test]
fn the_picker_filter_keeps_the_first_typed_character() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "tool.place", json!({}));
    assert!(
        h.state().shell().picker().is_some(),
        "tool.place opens the picker"
    );
    // No frame between the action and the typing: the first frame that paints the picker carries the text.
    h.event(egui::Event::Text("r".to_owned()));
    h.step();
    h.event(egui::Event::Text("elay".to_owned()));
    settle(&mut h);
    assert_eq!(h.state().shell().picker().unwrap().filter, "relay");
    h.state_mut().shell_mut().shutdown();
}

/// Cycle 38 (window check W38): with a navigator focused (it paints before the picker and takes typed text as its
/// type-ahead), the first key typed after `tool.place` still reaches the picker's filter ("relay", not "elay").
#[test]
fn the_picker_filter_keeps_the_first_typed_character_over_a_focused_navigator() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "navigator.show", json!({"kind": "connections"}));
    settle(&mut h);
    assert!(
        matches!(
            h.state().shell().focus(),
            Some(ew_ui::ew_viewmodel::shell::Focus::Navigator(_))
        ),
        "the navigator has the focus"
    );
    dispatch(&mut h, "tool.place", json!({}));
    h.event(egui::Event::Text("r".to_owned()));
    h.step();
    h.event(egui::Event::Text("elay".to_owned()));
    settle(&mut h);
    assert_eq!(h.state().shell().picker().unwrap().filter, "relay");
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn pl8_exact_counts_of_place_move_and_the_picker_in_the_window() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    settle(&mut h);
    let calls = h.state().shell().session_calls();
    let memo = h.state().shell().memo_totals().unwrap();
    // Pointer moves over the canvas with the place tool armed, as the window dispatches them (no frame in between,
    // so the window's dwell timer does not fire): no Session read, no memo re-execution.
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    for i in 0..30i64 {
        let p = (w / 4 + i * 1_300_000, hh / 3 + (i % 5) * 900_000);
        dispatch(
            &mut h,
            "canvas.pointer",
            json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6}),
        );
    }
    let _ = screen(&h, (w / 2, hh / 2));
    // Key transforms of the ghost.
    for action in [
        "tool.rotate_cw",
        "tool.mirror_x",
        "tool.next_variant",
        "tool.mirror_y",
    ] {
        dispatch(&mut h, action, json!({}));
    }
    let after = h.state().shell().session_calls();
    assert_eq!(after.page_content, calls.page_content, "{after:?}");
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.page_cps, calls.page_cps);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
    assert_eq!(after.place_catalogue, calls.place_catalogue);
    assert_eq!(
        h.state().shell().memo_totals().unwrap().1,
        memo.1,
        "no memo re-execution"
    );
    // A dwell that changes the command previews exactly once.
    dispatch(&mut h, "canvas.dwell", json!({"ms": 80}));
    dispatch(&mut h, "canvas.dwell", json!({"ms": 200}));
    let previewed = h.state().shell().session_calls();
    assert_eq!(previewed.preview_with, after.preview_with + 1);
    // A commit: exactly one commit (of the fresh preview) or execute; the next frame reads the page once and the
    // floating symbol's shape once (a new revision).
    dispatch(&mut h, "canvas.press", json!({"x_mm": 120.0, "y_mm": 120.0}));
    let committed = h.state().shell().session_calls();
    assert_eq!(
        committed.commit + committed.execute_with,
        previewed.commit + previewed.execute_with + 1
    );
    settle(&mut h);
    let next = h.state().shell().session_calls();
    assert_eq!(next.page_content, previewed.page_content + 1, "{next:?}");
    assert_eq!(next.page_tables, previewed.page_tables + 1);
    assert_eq!(next.symbol_shape, previewed.symbol_shape + 1);
    assert_eq!(
        next.run_script, previewed.run_script,
        "the window commits through the session, not a log"
    );
    // The picker: filtering and its row keys make no Session call.
    press(&mut h, Key::Escape);
    press(&mut h, Key::I);
    let opened = h.state().shell().session_calls();
    for text in ["m", "c", "b"] {
        h.get_all_by_role(egui::Role::TextInput)
            .next()
            .expect("the filter field")
            .type_text(text);
        h.run_ok();
        h.key_press(Key::ArrowDown);
        h.run_ok();
    }
    assert_eq!(
        h.state().shell().session_calls(),
        opened,
        "the picker filters without a Session call"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Four button events at one point, all in one frame: a double click by the window's clock (no sleep decides it).
fn double_click(h: &mut Harness<'_, App>, pos: Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    for pressed in [true, false, true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    settle(h);
}

/// The cycle-38 double-click rule: the Place tool defines no finish on a double click, so two quick clicks are two
/// plain presses and place twice (the window used to turn the second into `tool.finish` and place once).
#[test]
fn two_quick_clicks_with_the_place_tool_place_twice() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    settle(&mut h);
    let before = reps(&h);
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    let pos = screen(&h, (w * 3 / 4, hh * 3 / 4));
    double_click(&mut h, pos);
    let sent = h.state().dispatched();
    assert!(!sent.contains(&"tool.finish"), "{sent:?}");
    assert_eq!(
        sent.iter().filter(|a| **a == "canvas.press").count(),
        2,
        "two plain presses: {sent:?}"
    );
    assert_eq!(reps(&h), before + 2, "one placement per click");
    assert_eq!(h.state().shell().tool().name(), "place");
    h.state_mut().shell_mut().shutdown();
}

/// The Wire tool finishes on a double click (FT-708, doc 16's keymap): the second press is `tool.finish`, its release
/// is dropped, and the tool sees one press.
#[test]
fn a_double_click_with_the_wire_tool_is_its_finish() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "tool.wire", json!({}));
    settle(&mut h);
    assert!(h.state().shell().tool().finishes_on_double_click());
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    let pos = screen(&h, (w * 3 / 4, hh * 3 / 4));
    double_click(&mut h, pos);
    let sent = h.state().dispatched();
    assert!(sent.contains(&"tool.finish"), "{sent:?}");
    assert_eq!(
        sent.iter().filter(|a| **a == "canvas.press").count(),
        1,
        "the second press is the finish: {sent:?}"
    );
    assert_eq!(h.state().shell().tool().name(), "wire");
    h.state_mut().shell_mut().shutdown();
}

/// FT-605: in a project without terminal strips, choosing the terminal symbol in the picker opens the strip chooser
/// on its empty state, which says why and offers New terminal strip; the button creates a strip, which the chooser
/// lists as its current row, and choosing it arms the Place tool.
#[test]
fn ft605_a_terminal_without_strips_says_why_and_offers_a_new_strip() {
    use ew_ui::ew_viewmodel::shell::{Mode, ShellModel};
    use ew_ui::ew_viewmodel::tools::PickerMode;
    let tmp = Tmp::new();
    let dir = tmp.0.join("blank.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    for (action, params) in [
        (
            "file.new",
            json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
        ),
        (
            "page.new",
            json!({"folio": "101", "type": "multi_line", "title": "Power"}),
        ),
    ] {
        assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
    }
    let mut h: Harness<'static, App> = support::harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "tool.place", json!({}));
    dispatch(&mut h, "picker.filter", json!({"text": "ew.iec.terminal@"}));
    settle(&mut h);
    // Chosen in the picker, the terminal opens the empty strip chooser instead of doing nothing.
    dispatch(&mut h, "picker.choose", json!({}));
    settle(&mut h);
    let picker = h.state().shell().picker().expect("the strip chooser is open");
    assert_eq!(picker.mode, PickerMode::Strip);
    assert!(picker.rows.is_empty());
    assert!(
        h.get_all_by_role(egui::Role::Group)
            .any(|n| n.accesskit_node().label().as_deref() == Some("No terminal strips in this project yet")),
        "the empty state says why"
    );
    h.get_by_label("New terminal strip…").click();
    settle(&mut h);
    settle(&mut h);
    let picker = h.state().shell().picker().expect("the chooser stays open");
    assert_eq!(picker.mode, PickerMode::Strip);
    assert_eq!(picker.rows.len(), 1, "the new strip is listed");
    dispatch(&mut h, "picker.choose", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "place");
    h.state_mut().shell_mut().shutdown();
}
