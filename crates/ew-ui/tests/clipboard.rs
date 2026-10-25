// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard glue of the window without rendering (WP-S2.05 part 3, §8.5 and §8.7): on the canvas, the platform's
//! copy event runs `edit.copy` and the fragment goes to egui's `copy_text`; the paste event runs `edit.paste_clip`
//! with the system clipboard's text and arms the paste tool; Ctrl+D duplicates; Escape ends the tool. Keyboard and
//! clipboard events only.

mod support;

use std::sync::Arc;

use egui::{Key, Modifiers, OutputCommand};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::clipboard::ClipKind;
use ew_ui::ew_viewmodel::ew_api::output::{PageTables, parse_fragment};
use ew_ui::ew_viewmodel::shell::{Focus, Outcome, SessionCalls};
use ew_ui::ew_viewmodel::strings;
use ew_ui::ew_viewmodel::tools::Selection;
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
    h.get_by_label_contains("Paste").click();
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

const SENTINEL: &str = "cycle28-clipboard-sentinel";

/// Every state member the lossless Cut guard must preserve; feedback itself is checked separately.
#[derive(Debug, PartialEq, Eq)]
struct CutState {
    revision: Option<u64>,
    recorded: usize,
    selection: Selection,
    tables: Arc<PageTables>,
    clipboard: (Option<String>, Option<ClipKind>, usize),
    announcement: Option<String>,
    calls: SessionCalls,
}

fn cut_state(h: &Harness<'_, App>) -> CutState {
    let shell = h.state().shell();
    CutState {
        revision: shell.revision(),
        recorded: shell.recorded_commands().len(),
        selection: shell.selection().clone(),
        tables: shell.canvas().unwrap().page().tables.clone(),
        clipboard: (
            shell.clipboard().text().map(str::to_owned),
            shell.clipboard().kind().cloned(),
            shell.clipboard().dropped(),
        ),
        announcement: shell.tool_announcement().map(str::to_owned),
        calls: shell.session_calls(),
    }
}

fn prime_clipboard(h: &mut Harness<'_, App>) {
    h.event(egui::Event::Paste(SENTINEL.into()));
    settle(h);
    assert_eq!(h.state().shell().clipboard().text(), Some(SENTINEL));
    assert!(h.state().shell().banner().is_none());
    assert_eq!(h.state().shell().tool().name(), "select");
    // Setup is complete before the action frame whose platform output the test will inspect.
    let _ = h.state_mut().shell_mut().take_clipboard_outbox();
    settle(h);
}

/// A marker alone cannot be cut losslessly because its wire is outside the selection.
fn incomplete_marker(h: &mut Harness<'_, App>) {
    let tables = h.state().shell().canvas().unwrap().page().tables.clone();
    let connection = tables.connections.first().expect("a wire on 101").id;
    dispatch(
        h,
        "command.run",
        json!({"cmd": "marker.place", "params": {
            "page": tables.page.to_string(), "connections": [connection.to_string()], "at": [100_000_000, 100_000_000]
        }}),
    );
    let id = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .items
        .iter()
        .find(|i| i.kind == "wire_marker" && !tables.items.iter().any(|old| old.id == i.id))
        .expect("the newly placed marker")
        .id;
    dispatch(h, "select.set", json!({"ids": [id.to_string()]}));
    settle(h);
    assert!(matches!(h.state().shell().focus(), Some(Focus::Canvas(_))));
    assert_eq!(h.state().shell().selection().items.len(), 1);
    assert_eq!(h.state().shell().selection().items[0].0, id);
    prime_clipboard(h);
}

fn assert_no_copy_output(h: &Harness<'_, App>) {
    assert!(
        !h.output()
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, OutputCommand::CopyText(_))),
        "Cut must not publish a partial fragment"
    );
}

fn assert_cut_refusal(h: &mut Harness<'_, App>, mut before: CutState) {
    before.calls.copy_fragment += 1;
    settle(h);
    assert_eq!(cut_state(h), before, "only copy preparation may change");
    let reason = strings::text("reason-cut-incomplete-items");
    assert_eq!(h.state().shell().banner(), Some(reason));
    assert!(
        h.query_all_by_label(reason).next().is_some(),
        "the explanation is accessible"
    );
    settle(h);
    assert_eq!(
        h.state().shell().banner(),
        Some(reason),
        "another ordinary frame keeps the explanation"
    );
    assert!(h.query_all_by_label(reason).next().is_some());
    assert_eq!(cut_state(h), before);
}

#[test]
fn incomplete_cut_clipboard_event_shows_reason_without_changes() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    incomplete_marker(&mut h);
    let before = cut_state(&h);
    h.event(egui::Event::Cut);
    h.step();
    assert_no_copy_output(&h);
    assert_cut_refusal(&mut h, before);
}

#[test]
fn incomplete_cut_menu_shows_reason_without_changes() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    incomplete_marker(&mut h);
    h.get_by_label("Edit").click();
    settle(&mut h);
    assert!(!h.get_by_label_contains("Cut").accesskit_node().is_disabled());
    let before = cut_state(&h);
    h.get_by_label_contains("Cut").click();
    h.step();
    assert_no_copy_output(&h);
    assert_cut_refusal(&mut h, before);
}

#[test]
fn successful_page_item_cut_has_no_refusal_banner() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let page = h.state().shell().canvas().unwrap().page().tables.page.to_string();
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "item.place", "params": {
            "page": page, "kind": {"free_text": {"text": "40x60", "at": [100_000_000, 100_000_000]}}
        }}),
    );
    let id = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .items
        .iter()
        .find(|i| i.kind == "free_text" && i.text.as_deref() == Some("40x60"))
        .expect("the free text")
        .id;
    dispatch(&mut h, "select.set", json!({"ids": [id.to_string()]}));
    settle(&mut h);
    assert!(matches!(h.state().shell().focus(), Some(Focus::Canvas(_))));
    prime_clipboard(&mut h);
    let before = cut_state(&h);
    h.event(egui::Event::Cut);
    h.step();
    let copied = h
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .expect("successful Cut publishes the fragment");
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.revision(), before.revision.map(|r| r + 1));
    assert_eq!(shell.recorded_commands().len(), before.recorded + 1);
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "edit.delete");
    assert!(
        !shell
            .canvas()
            .unwrap()
            .page()
            .tables
            .items
            .iter()
            .any(|i| i.id == id)
    );
    assert_eq!(shell.clipboard().text(), Some(copied.as_str()));
    assert_eq!(
        shell.clipboard().kind(),
        Some(&ClipKind::Fragment {
            records: 1,
            copies: 0
        })
    );
    assert_eq!(shell.clipboard().dropped(), 0);
    let fragment = parse_fragment(&copied).unwrap();
    assert_eq!(fragment.records, 1);
    let record = &fragment.params["records"][0];
    assert_eq!(record["id"], id.to_string());
    assert_eq!(record["$k"], "page_item");
    assert_eq!(record["kind"]["free_text"]["text"], "40x60");
    assert!(shell.banner().is_none());
}

#[test]
fn plain_text_paste_refusal_remains_quiet() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    assert!(matches!(h.state().shell().focus(), Some(Focus::Canvas(_))));
    let rev = h.state().shell().revision();
    h.event(egui::Event::Paste("ordinary external text".into()));
    settle(&mut h);
    assert!(h.state().shell().banner().is_none());
    assert_eq!(h.state().shell().tool().name(), "select");
    assert_eq!(h.state().shell().revision(), rev);
}
