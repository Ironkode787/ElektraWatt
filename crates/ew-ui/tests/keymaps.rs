// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 2 (keymaps) in the window without rendering: KM-1 with the KiCad-like preset the keys A, G, W and E
//! run Place, Move, Wire and Properties on the canvas (and A does nothing in the EW preset); KM-5 Ctrl+Shift+N runs
//! Renumber devices…; KM-2 a user keymap's chord runs its action; KM-3 the editor records a pressed chord, shows a
//! conflict as an error and a shadowing note. The accessible-name walker runs after every step.

mod support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

/// The window on a writable copy of Golden A, folio 103, the canvas focused, with `settings` and `keymap` written to
/// the settings directory first.
fn on_golden(tmp: &Tmp, settings: &str, keymap: Option<&str>) -> Harness<'static, App> {
    let config = tmp.0.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("settings.toml"), settings).unwrap();
    if let Some(k) = keymap {
        std::fs::write(config.join("keymap.toml"), k).unwrap();
    }
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "103" }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h.state_mut().shell_mut().set_dispatch_log(true);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// The actions the window dispatched for one key press (in order).
fn actions_of(h: &mut Harness<'_, App>, modifiers: Modifiers, key: Key) -> Vec<String> {
    let _ = h.state_mut().shell_mut().take_dispatch_log();
    h.key_press_modifiers(modifiers, key);
    settle(h);
    let log: Vec<String> = h
        .state_mut()
        .shell_mut()
        .take_dispatch_log()
        .into_iter()
        .map(|e| e.action)
        .collect();
    // Back to the Select tool for the next key.
    let _ = h.state_mut().shell_mut().dispatch("ui.dismiss", &json!({}));
    let _ = h.state_mut().shell_mut().dispatch("ui.dismiss", &json!({}));
    settle(h);
    log
}

const FIRST_RUN_DONE: &str = "first_run_done = true\n";

/// KM-1 (FR-UI-013): the KiCad-like preset in the window: A starts Place, G Move, W Wire and E focuses Properties on
/// the canvas.
#[test]
fn km1_the_kicad_preset_binds_a_g_w_and_e_on_the_canvas() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "first_run_done = true\nkeymap = \"kicad\"\n", None);
    for (key, action) in [
        (Key::A, "tool.place"),
        (Key::G, "tool.move"),
        (Key::W, "tool.wire"),
        (Key::E, "props.focus"),
        (Key::M, "tool.move"),
    ] {
        let log = actions_of(&mut h, Modifiers::NONE, key);
        assert_eq!(log.first().map(String::as_str), Some(action), "{key:?}: {log:?}");
    }
    let log = actions_of(&mut h, Modifiers::NONE, Key::I);
    assert!(
        !log.iter().any(|a| a == "tool.place"),
        "I is not Place in the KiCad-like preset: {log:?}"
    );
}

/// KM-1: in the EW preset A is not bound and I starts Place.
#[test]
fn km1_the_ew_preset_keeps_i_for_place() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, FIRST_RUN_DONE, None);
    let log = actions_of(&mut h, Modifiers::NONE, Key::A);
    assert!(log.is_empty(), "{log:?}");
    let log = actions_of(&mut h, Modifiers::NONE, Key::I);
    assert_eq!(log.first().map(String::as_str), Some("tool.place"), "{log:?}");
}

/// KM-5 (D7): Ctrl+Shift+N runs Renumber devices….
#[test]
fn km5_ctrl_shift_n_runs_renumber_devices() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, FIRST_RUN_DONE, None);
    let log = actions_of(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::N);
    assert_eq!(
        log.first().map(String::as_str),
        Some("devices.renumber"),
        "{log:?}"
    );
}

/// KM-2: a user keymap's chord runs its action in the window, and the preset's key of that action no longer does.
#[test]
fn km2_a_user_binding_replaces_the_presets_in_the_window() {
    let tmp = Tmp::new();
    let mut h = on_golden(
        &tmp,
        FIRST_RUN_DONE,
        Some(
            "[meta]\nname = \"Mine\"\nversion = 1\n\n[[bind]]\nkeys = \"Ctrl+Shift+W\"\naction = \"tool.wire\"\n\
             context = \"canvas\"\n",
        ),
    );
    assert_eq!(h.state().shell().banner(), None);
    let log = actions_of(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::W);
    assert_eq!(log.first().map(String::as_str), Some("tool.wire"), "{log:?}");
    let log = actions_of(&mut h, Modifiers::NONE, Key::W);
    assert!(!log.iter().any(|a| a == "tool.wire"), "{log:?}");
}

/// KM-3 in the window: Record, then a pressed chord sets the binding; a conflict shows as an error line; the row of a
/// canvas binding over a global one shows its shadowing; the walker passes with the editor open.
#[test]
fn km3_the_editor_records_a_pressed_chord_and_shows_conflicts_and_shadowing() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, FIRST_RUN_DONE, None);
    dispatch(&mut h, "help.shortcuts", json!({}));
    settle(&mut h);
    assert!(
        h.query_by_label("Search actions or keys").is_some()
            || h.query_by_label_contains("Preset: EW").is_some()
    );
    dispatch(&mut h, "keymap.search", json!({ "text": "connection" }));
    settle(&mut h);
    let first = h.state().shell().keymap_rows()[0].action;
    h.get_all_by_label("Record")
        .next()
        .expect("a Record button")
        .click();
    settle(&mut h);
    assert_eq!(h.state().shell().keymap_editor().recording, Some(first));
    // The next key press is the chord.
    dispatch(&mut h, "keymap.record_start", json!({ "action": "tool.wire" }));
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::W);
    settle(&mut h);
    let wire = ew_ui::ew_viewmodel::actions::find("tool.wire").unwrap();
    assert_eq!(h.state().shell().shortcuts_of(wire), ["Ctrl+Shift+W"]);
    assert!(h.state().shell().keymap_editor_open(), "the dialog stays open");
    // Shadowing: the row says what the canvas binding shadows.
    assert!(
        h.query_by_label_contains("shadows Close project").is_some(),
        "the shadowing note"
    );
    // A conflict: refused, shown as an error line.
    let outcome = h
        .state_mut()
        .shell_mut()
        .dispatch("keymap.record", &json!({ "action": "tool.wire", "keys": "M" }));
    assert!(matches!(outcome, Outcome::Refused { .. }), "{outcome:?}");
    settle(&mut h);
    assert!(
        h.query_by_label_contains("M is already Move in the canvas.")
            .is_some()
    );
}
