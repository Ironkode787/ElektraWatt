// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keymaps in the window without rendering: with the KiCad-like preset the keys A, G, W and E
//! run Place, Move, Wire and Properties on the canvas (and A does nothing in the EW preset); Ctrl+Shift+N runs
//! Renumber devices…; a user keymap's chord runs its action; the editor records a pressed chord, shows a
//! conflict as an error and a shadowing note. The accessible-name walker runs after every step.

use crate::support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::App;
use ew_viewmodel::shell::Outcome;
use serde_json::json;
use support::{Tmp, dispatch, settle};

/// The window on a writable copy of the sample, folio 103, the canvas focused, with `settings` and `keymap` written to
/// the settings directory first.
fn on_sample(tmp: &Tmp, settings: &str, keymap: Option<&str>) -> Harness<'static, App> {
    let config = tmp.0.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("settings.toml"), settings).unwrap();
    if let Some(k) = keymap {
        std::fs::write(config.join("keymap.toml"), k).unwrap();
    }
    let mut h = support::on_sample(tmp, "103");
    h.state_mut().shell_mut().set_dispatch_log(true);
    h
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

/// The KiCad-like preset in the window: A starts Place, G Move, W Wire and E focuses Properties on
/// the canvas.
#[test]
fn the_kicad_preset_binds_a_g_w_and_e_on_the_canvas() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "first_run_done = true\nkeymap = \"kicad\"\n", None);
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

/// In the EW preset A is not bound and I starts Place. Ctrl+Shift+N runs Renumber devices….
#[test]
fn the_ew_preset_keeps_i_for_place_and_binds_ctrl_shift_n_to_renumber() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, FIRST_RUN_DONE, None);
    let log = actions_of(&mut h, Modifiers::NONE, Key::A);
    assert!(log.is_empty(), "{log:?}");
    let log = actions_of(&mut h, Modifiers::NONE, Key::I);
    assert_eq!(log.first().map(String::as_str), Some("tool.place"), "{log:?}");
    let log = actions_of(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::N);
    assert_eq!(
        log.first().map(String::as_str),
        Some("devices.renumber"),
        "{log:?}"
    );
}

/// A user keymap's chord runs its action in the window, and the preset's key of that action no longer does.
#[test]
fn a_user_binding_replaces_the_presets_in_the_window() {
    let tmp = Tmp::new();
    let mut h = on_sample(
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

/// In the window: Record, then a pressed chord sets the binding; a conflict shows as an error line; the row of a
/// canvas binding over a global one shows its shadowing; the walker passes with the editor open.
#[test]
fn the_editor_records_a_pressed_chord_and_shows_conflicts_and_shadowing() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, FIRST_RUN_DONE, None);
    dispatch(&mut h, "help.shortcuts", json!({}));
    settle(&mut h);
    assert!(
        h.query_by_label("Search actions or keys").is_some()
            || h.query_by_label_contains("Preset: EW").is_some()
    );
    dispatch(&mut h, "keymap.search", json!({ "text": "wire" }));
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
    let wire = ew_viewmodel::actions::find("tool.wire").unwrap();
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

/// `ui.dismiss` from a key while the app menu is open closes the menu and nothing else, as
/// Escape does; with the menu closed the same key dismisses as before. A physical key of the user's keymap reaches the
/// window's event loop with `ui.dismiss`; a logical key does too (the logical-key loop matches a `ui.dismiss` bound to
/// a key other than Escape as the physical-key loop does).
#[test]
fn ui_dismiss_with_the_app_menu_open_closes_only_the_menu() {
    for (keys, modifiers, key) in [
        ("phys:Backslash", Modifiers::NONE, Key::Backslash),
        ("Alt+Q", Modifiers::ALT, Key::Q),
    ] {
        let tmp = Tmp::new();
        let keymap = format!(
            "[meta]\nname = \"Mine\"\nversion = 1\n\n[[bind]]\nkeys = \"{keys}\"\naction = \"ui.dismiss\"\n\
             context = \"global\"\n"
        );
        let mut h = on_sample(&tmp, FIRST_RUN_DONE, Some(&keymap));
        assert_eq!(h.state().shell().banner(), None, "{keys}");
        h.key_press(Key::F10);
        settle(&mut h);
        assert!(
            support::off_chrome(&h, "File").is_some(),
            "{keys}: F10 opens the app menu"
        );
        let log = actions_of(&mut h, modifiers, key);
        assert!(
            support::off_chrome(&h, "File").is_none(),
            "{keys}: the menu closed: {log:?}"
        );
        assert!(log.is_empty(), "{keys}: nothing else ran: {log:?}");
        let log = actions_of(&mut h, modifiers, key);
        assert_eq!(
            log.first().map(String::as_str),
            Some("ui.dismiss"),
            "{keys}: {log:?}"
        );
    }
}

/// The keys that stood outside the keymap are its bindings: help and the left rail's entries take the keys a user
/// keymap gives them, and their default keys then do nothing.
#[test]
fn a_rebound_help_and_rail_key_hold_and_the_default_keys_go() {
    let tmp = Tmp::new();
    let mut h = on_sample(
        &tmp,
        FIRST_RUN_DONE,
        Some(
            "[meta]\nname = \"Mine\"\nversion = 1\n\n[[bind]]\nkeys = \"Shift+F12\"\naction = \"help.documentation\"\n\
             context = \"global\"\n\n[[bind]]\nkeys = \"Alt+1\"\nalt = \"Alt+2\"\naction = \"rail.entry\"\n\
             context = \"global\"\n",
        ),
    );
    assert_eq!(h.state().shell().banner(), None);
    let log = actions_of(&mut h, Modifiers::SHIFT, Key::F12);
    assert!(log.iter().any(|a| a == "help.documentation"), "{log:?}");
    let log = actions_of(&mut h, Modifiers::NONE, Key::F1);
    assert!(!log.iter().any(|a| a == "help.documentation"), "{log:?}");
    let log = actions_of(&mut h, Modifiers::ALT, Key::Num2);
    assert_eq!(log.first().map(String::as_str), Some("rail.entry"), "{log:?}");
    let log = actions_of(&mut h, Modifiers::COMMAND, Key::Num2);
    assert!(!log.iter().any(|a| a == "rail.entry"), "{log:?}");
}

/// F10 for the app menu and Shift+F10 for a canvas's context menu were keys the window read itself, outside the
/// keymap, so they could not be changed. They are bindings of the window's own actions (`ui.app_menu`,
/// `ui.context_menu`): bound to other keys, the new keys open the menus and the old ones no longer do.
#[test]
fn the_app_menu_and_the_context_menu_keys_are_bindings_a_person_can_change() {
    let tmp = Tmp::new();
    let mut h = on_sample(
        &tmp,
        FIRST_RUN_DONE,
        Some(
            "[meta]\nname = \"Mine\"\nversion = 1\n\n[[bind]]\nkeys = \"F9\"\naction = \"ui.app_menu\"\n\
             context = \"global\"\n\n[[bind]]\nkeys = \"Ctrl+F9\"\naction = \"ui.context_menu\"\ncontext = \"canvas\"\n",
        ),
    );
    assert_eq!(h.state().shell().banner(), None);
    let menu_open = |h: &Harness<'_, App>| support::off_chrome(h, "File").is_some();
    h.key_press(Key::F10);
    settle(&mut h);
    assert!(!menu_open(&h), "F10 is no longer the app menu's");
    h.key_press(Key::F9);
    settle(&mut h);
    settle(&mut h);
    assert!(menu_open(&h), "F9 opens the app menu");
    h.key_press(Key::Escape);
    settle(&mut h);
    // A selected symbol's menu at the selection, by the new key.
    let rep = h.state().shell().canvas().unwrap().page().tables.reps[0]
        .id
        .to_string();
    dispatch(&mut h, "select.set", json!({ "ids": [rep] }));
    dispatch(&mut h, "ui.focus", json!({ "area": "canvas" }));
    settle(&mut h);
    let menus = |h: &Harness<'_, App>| h.query_all(egui_kittest::kittest::by()).count();
    let closed = menus(&h);
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(menus(&h), closed, "Shift+F10 is no longer the context menu's");
    h.key_press_modifiers(Modifiers::COMMAND, Key::F9);
    settle(&mut h);
    settle(&mut h);
    assert!(menus(&h) > closed, "Ctrl+F9 opens the canvas's context menu");
}

/// Shift+F10 with nothing selected opens no menu, and the request goes with its frame: a later selection on the
/// canvas does not open the context menu (it did, from a request left standing).
#[test]
fn a_context_menu_key_with_nothing_selected_opens_no_menu_later() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, FIRST_RUN_DONE, None);
    dispatch(&mut h, "select.set", json!({ "ids": [] }));
    dispatch(&mut h, "ui.focus", json!({ "area": "canvas" }));
    settle(&mut h);
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    settle(&mut h);
    settle(&mut h);
    assert!(!egui::Popup::is_any_open(&h.ctx), "nothing selected: no menu");
    let rep = h.state().shell().canvas().unwrap().page().tables.reps[0]
        .id
        .to_string();
    dispatch(&mut h, "select.set", json!({ "ids": [rep] }));
    dispatch(&mut h, "ui.focus", json!({ "area": "canvas" }));
    settle(&mut h);
    settle(&mut h);
    assert!(!egui::Popup::is_any_open(&h.ctx), "a later selection opens no menu");
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    settle(&mut h);
    settle(&mut h);
    assert!(egui::Popup::is_any_open(&h.ctx), "Shift+F10 with the symbol selected opens its menu");
}
