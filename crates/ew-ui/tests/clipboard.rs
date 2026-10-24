// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard glue of the window without rendering (WP-S2.05 part 3, §8.5 and §8.7): on the canvas, the platform's
//! copy event runs `edit.copy` and the fragment goes to egui's `copy_text`; the paste event runs `edit.paste_clip`
//! with the system clipboard's text and arms the paste tool; Ctrl+D duplicates; Escape ends the tool. Keyboard and
//! clipboard events only.

mod support;

use egui::{Key, Modifiers, OutputCommand};
use egui_kittest::Harness;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Focus, Outcome};
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

/// Selects -102F1's representation on folio 102.
fn select_102f1(h: &mut Harness<'_, App>) {
    dispatch(h, "select.set", json!({"refs": ["tag:-102F1"]}));
    let shell = h.state().shell();
    let dev = shell.selection().items[0].0;
    let rep = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device == Some(dev))
        .unwrap()
        .id
        .to_string();
    dispatch(h, "select.set", json!({ "ids": [rep] }));
}

#[test]
fn copy_and_paste_events_reach_the_clipboard_actions_on_the_canvas() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    select_102f1(&mut h);
    assert!(matches!(h.state().shell().focus(), Some(Focus::Canvas(_))));
    // The platform's copy event (Ctrl+C under winit): the fragment to the shell's clipboard and to `copy_text`.
    h.event(egui::Event::Copy);
    h.step();
    let copied = h
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .expect("the frame hands the fragment to the system clipboard");
    assert!(copied.starts_with("#ew-fragment v1\n"));
    assert_eq!(h.state().shell().clipboard().text(), Some(copied.as_str()));
    settle(&mut h);
    // The paste event (Ctrl+V) with the system clipboard's text arms the paste tool.
    h.event(egui::Event::Paste(copied.clone()));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "paste");
    // Escape ends it; nothing is committed.
    let rev = h.state().shell().revision();
    h.key_press(Key::Escape);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert_eq!(h.state().shell().revision(), rev);
    // Plain text from another application is refused quietly.
    h.event(egui::Event::Paste("hello".into()));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    // Ctrl+D duplicates the selection: the paste tool floats a copy, the clipboard keeps the text.
    select_102f1(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::D);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "paste");
    assert_eq!(h.state().shell().clipboard().text(), Some("hello"));
    // Enter pastes at the ghost.
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1));
    assert_eq!(h.state().shell().tool().name(), "select");
    assert_eq!(h.state().shell().selection().items.len(), 1);
}

/// A focused text field keeps egui's own copy and paste: the gate `ctx.text_edit_focused()` in `route_keys` leaves
/// the platform's paste and copy events to the field, so no tool is armed and the shell's clipboard is not written.
#[test]
fn text_fields_keep_their_own_copy_and_paste() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    h.key_press_modifiers(Modifiers::COMMAND, Key::F);
    settle(&mut h);
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    let rev = h.state().shell().revision();
    assert_eq!(h.state().shell().clipboard().text(), None);
    h.event(egui::Event::Paste("x".into()));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert_eq!(h.state().shell().revision(), rev);
    assert!(h.state().shell().find_state().text().contains('x'));
    h.event(egui::Event::Copy);
    settle(&mut h);
    assert_eq!(h.state().shell().clipboard().text(), None);
}

/// Edit > Paste asks the platform for the system clipboard (`ViewportCommand::RequestPaste`, which eframe answers
/// with a paste event) instead of reusing the shell's last text.
#[test]
fn menu_paste_requests_the_system_clipboard() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    select_102f1(&mut h);
    h.event(egui::Event::Copy);
    settle(&mut h);
    let text = h.state().shell().clipboard().text().unwrap().to_owned();
    h.get_by_label("Edit").click();
    settle(&mut h);
    h.get_by_label("Paste").click();
    h.step();
    assert!(
        h.output().viewport_output.values().any(|v| v
            .commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::RequestPaste))),
        "the frame asks the platform for the clipboard"
    );
    assert_eq!(h.state().shell().tool().name(), "select");
    // The platform's answer.
    h.event(egui::Event::Paste(text));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "paste");
}
