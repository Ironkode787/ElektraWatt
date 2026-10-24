// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard through the shell on temporary copies of Golden A (WP-S2.05 part 3, acceptance checks CB4, CB7 and
//! the shell half of CB8): FR-DEV-029's copy and paste of -102F1 on folio 102 (-102F3, part A9F78116, no wire); the
//! paste tool (ghost, dwell preview with the predicted tags, commit with the pasted representation selected, Escape);
//! duplicate (the clipboard unchanged); cut (one undo step, the clipboard keeps the fragment); plain text refused; the
//! transcript's `clipboard` and paste `ghost` members, the describe lines with the 6 words, and the exact counts.

mod common;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`.
fn writable(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

/// A device selected by its tag (`select.set`, the scripts' and the blind drafter's way) is copied, and cut, as its
/// representations on the page shown.
#[test]
fn a_device_selected_by_tag_is_copied_as_its_representation() {
    let tmp = Tmp::new("clip-by-tag");
    let mut shell = writable(&tmp, "102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    done(&mut shell, "edit.copy", json!({}));
    assert_eq!(
        state(&shell)["clipboard"],
        json!({"kind": "fragment", "records": 4, "copies": 3})
    );
    let reps = shell.canvas().unwrap().page().tables.reps.len();
    done(&mut shell, "edit.cut", json!({}));
    assert_eq!(shell.canvas().unwrap().page().tables.reps.len(), reps - 1);
    shell.shutdown();
}

/// Selects the representation of the device printed `tag` on the page shown; its ID text.
fn select_rep(shell: &mut ShellModel, tag: &str) -> String {
    done(shell, "select.set", json!({"refs": [format!("tag:{tag}")]}));
    let dev = shell.selection().items[0].0;
    let rep = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device == Some(dev))
        .expect("the device is drawn here")
        .id
        .to_string();
    done(shell, "select.set", json!({ "ids": [rep] }));
    rep
}

/// The sheet point of the page point `p` on the page shown.
fn sheet(shell: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    shell.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The page position of the representation `rep` on the page shown.
fn rep_pos(shell: &ShellModel, rep: &str) -> (i64, i64) {
    shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.id.to_string() == rep)
        .unwrap()
        .pos
}

/// The QET grid step in nm (rounded): a sheet offset that moves the snapped anchor by one step.
const STEP: i64 = 2_645_833;

/// CB4 (FR-DEV-029): copying -102F1 on folio 102 and pasting it on 102 yields a device whose tag is -102F3 with part
/// A9F78116 and no connection (its wires had an end outside).
#[test]
fn cb4_copy_and_paste_of_102f1_is_102f3_with_its_part_and_no_wire() {
    let tmp = Tmp::new("clip-cb4");
    let mut shell = writable(&tmp, "102");
    let rep = select_rep(&mut shell, "-102F1");
    done(&mut shell, "edit.copy", json!({}));
    assert_eq!(
        state(&shell)["clipboard"],
        json!({"kind": "fragment", "records": 4, "copies": 3})
    );
    let wires = shell.canvas().unwrap().page().tables.connections.len();
    let reps = shell.canvas().unwrap().page().tables.reps.len();
    done(&mut shell, "edit.paste_clip", json!({}));
    let target = sheet(&shell, rep_pos(&shell, &rep));
    // Six grid steps down: an empty spot below the original.
    done(
        &mut shell,
        "canvas.pointer",
        at((target.0, target.1 + 6 * STEP), json!({})),
    );
    let rev = shell.revision();
    done(
        &mut shell,
        "canvas.press",
        at((target.0, target.1 + 6 * STEP), json!({})),
    );
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(shell.tool().name(), "select");
    assert_eq!(shell.selection().items.len(), 1, "the pasted representation");
    assert_ne!(shell.selection().items[0].0.to_string(), rep);
    assert_eq!(
        shell.canvas().unwrap().page().tables.connections.len(),
        wires,
        "no connection is pasted"
    );
    done(&mut shell, "props.show", json!({"on": true}));
    let st = state(&shell);
    assert_eq!(st["properties"]["title"], "-102F3");
    let sheet_ = shell.properties().sheet().unwrap();
    let part = find_row(sheet_, "device/parts[0].part").expect("the part row");
    match &part.1.value {
        RowValue::One { text, .. } => assert!(text.starts_with("A9F78116"), "{text}"),
        other => panic!("{other:?}"),
    }
    // One undo step removes the paste.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.canvas().unwrap().page().tables.reps.len(), reps);
    shell.shutdown();
}

/// CB7: the paste tool: the ghost follows the pointer (no `Session` call), a dwell previews `edit.paste` once with the
/// predicted tag, Escape ends the tool with the revision unchanged; the transcript, the describe lines and the counts.
#[test]
fn cb7_the_paste_tool_ghost_preview_and_escape() {
    let tmp = Tmp::new("clip-cb7");
    let mut shell = writable(&tmp, "102");
    let rep = select_rep(&mut shell, "-102F1");
    let calls = shell.session_calls();
    done(&mut shell, "edit.copy", json!({}));
    assert_eq!(shell.session_calls().copy_fragment, calls.copy_fragment + 1);
    let before = shell.session_calls();
    done(&mut shell, "edit.paste_clip", json!({}));
    let after = shell.session_calls();
    assert_eq!(after.parse_fragment, before.parse_fragment + 1, "one parse");
    assert_eq!(after.copy_fragment, before.copy_fragment);
    assert_eq!(shell.tool().name(), "paste");
    assert_eq!(
        state(&shell)["canvas"]["tool"],
        json!({"name": "paste", "state": "floating"})
    );
    let target = sheet(&shell, rep_pos(&shell, &rep));
    let p = (target.0, target.1 + 6 * STEP);
    let moved = shell.session_calls();
    done(&mut shell, "canvas.pointer", at(p, json!({})));
    done(&mut shell, "canvas.pointer", at((p.0 + 10, p.1), json!({})));
    assert_eq!(shell.session_calls(), moved, "ghost moves call nothing");
    let st = state(&shell);
    let ghost = &st["canvas"]["ghost"];
    assert_eq!(ghost["kind"], "paste");
    assert_eq!(ghost["records"], 4);
    assert_eq!(ghost["dropped"], 0);
    assert!(ghost["at_nm"].is_array());
    assert_eq!(st["canvas"]["overlay"], 2, "the ghost rectangles and the label");
    // The dwell: one preview with the predicted tag; a second dwell at the same anchor none.
    done(&mut shell, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(shell.session_calls().preview_with, moved.preview_with + 1);
    done(&mut shell, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(shell.session_calls().preview_with, moved.preview_with + 1);
    let st = state(&shell);
    assert_eq!(st["canvas"]["preview"]["command"], "edit.paste");
    assert_eq!(st["canvas"]["preview"]["state"], "ready");
    assert_eq!(st["canvas"]["preview"]["tags"], 1, "the new device's tag");
    assert!(
        shell.tool_overlay().labels().any(|l| l.contains("-102F3")),
        "the ghost shows the resulting tag: {:?}",
        shell.tool_overlay().labels().collect::<Vec<_>>()
    );
    // The describe lines and their words.
    let d = shell.describe();
    assert!(d.contains("clipboard fragment records 4 copies 3"), "{d}");
    let line = d
        .lines()
        .find(|l| l.starts_with("canvas ghost paste records 4 at "))
        .unwrap_or_else(|| panic!("{d}"));
    assert!(line.ends_with(" mm dropped 0"), "{line}");
    let shortcut_keys: Vec<String> = ew_viewmodel::actions::ACTIONS
        .iter()
        .flat_map(|a| a.shortcut.iter())
        .flat_map(|s| s.split(['+', ' ']))
        .map(str::to_owned)
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    for l in d.lines() {
        for word in structural_words(l, &shortcut_keys) {
            assert!(
                ew_viewmodel::describe::VOCABULARY.contains(&word.as_str()),
                "`{word}` is in no word list; line: {l}"
            );
            seen.insert(word);
        }
    }
    for w in ew_viewmodel::describe::CLIP_WORDS {
        assert!(ew_viewmodel::describe::VOCABULARY.contains(w), "{w}");
        assert!(seen.contains(*w), "{w} is reached");
    }
    assert_eq!(ew_viewmodel::describe::CLIP_WORDS.len(), 6);
    // Escape: the tool ends, nothing is committed.
    let rev = shell.revision();
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().name(), "select");
    assert_eq!(shell.revision(), rev);
    assert_eq!(state(&shell)["canvas"]["ghost"], Value::Null);
    shell.shutdown();
}

/// CB7: duplicate floats a copy two grid steps right and down of the selection's anchor and leaves the clipboard as
/// it was; Enter pastes it.
#[test]
fn cb7_duplicate_leaves_the_clipboard_and_starts_two_steps_off() {
    let tmp = Tmp::new("clip-dup");
    let mut shell = writable(&tmp, "102");
    done(&mut shell, "clipboard.text", json!({"text": "-101F1\n"}));
    let clip = state(&shell)["clipboard"].clone();
    assert_eq!(clip, json!({"kind": "text", "lines": 1}));
    select_rep(&mut shell, "-102F1");
    done(&mut shell, "edit.duplicate", json!({}));
    assert_eq!(
        state(&shell)["clipboard"],
        clip,
        "duplicate does not touch the clipboard"
    );
    assert_eq!(shell.tool().name(), "paste");
    let ghost = state(&shell)["canvas"]["ghost"].clone();
    // -102F1's anchor is [121708333, 52916667] (the fragment of fragment_102F1.txt); two QET steps on.
    assert_eq!(ghost["at_nm"], json!([127_000_000, 58_208_333]));
    let rev = shell.revision();
    let p = sheet(&shell, (127_000_000, 58_208_333));
    done(&mut shell, "canvas.press", at(p, json!({})));
    assert_eq!(shell.revision(), rev.map(|r| r + 1));
    assert_eq!(shell.tool().name(), "select");
    shell.shutdown();
}

/// CB7: cut copies and deletes in one undo step; the clipboard keeps the fragment, and undo restores the records.
#[test]
fn cb7_cut_is_one_undo_step_and_the_clipboard_keeps_the_fragment() {
    let tmp = Tmp::new("clip-cut");
    let mut shell = writable(&tmp, "102");
    let rep = select_rep(&mut shell, "-102F2");
    let rev = shell.revision();
    let steps = shell.recorded_commands().len();
    done(&mut shell, "edit.cut", json!({}));
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(shell.recorded_commands().len(), steps + 1);
    assert_eq!(shell.recorded_commands()[steps].cmd, "edit.delete");
    assert!(
        !shell
            .canvas()
            .unwrap()
            .page()
            .tables
            .reps
            .iter()
            .any(|r| r.id.to_string() == rep),
        "the representation is gone"
    );
    assert_eq!(state(&shell)["clipboard"]["kind"], "fragment");
    done(&mut shell, "edit.undo", json!({}));
    assert!(
        shell
            .canvas()
            .unwrap()
            .page()
            .tables
            .reps
            .iter()
            .any(|r| r.id.to_string() == rep),
        "undo restores it"
    );
    assert_eq!(state(&shell)["clipboard"]["kind"], "fragment");
    shell.shutdown();
}

/// CB7: plain text and an empty clipboard are refused; a damaged fragment shows in the banner; copy without a selection
/// is refused; the tag list goes to the clipboard as text.
#[test]
fn cb7_plain_text_is_refused_and_the_refusals() {
    let tmp = Tmp::new("clip-refusals");
    let mut shell = writable(&tmp, "101");
    assert_eq!(
        refused(&mut shell, "edit.paste_clip", json!({})),
        "reason-clipboard-empty"
    );
    assert_eq!(state(&shell)["clipboard"], Value::Null);
    assert!(shell.describe().contains("\nclipboard none\n"));
    done(&mut shell, "clipboard.text", json!({"text": "hello\nworld\n"}));
    assert_eq!(
        refused(&mut shell, "edit.paste_clip", json!({})),
        "reason-not-a-fragment"
    );
    assert_eq!(shell.tool().name(), "select");
    match shell.dispatch(
        "edit.paste_clip",
        &json!({"text": "#ew-fragment v1\n{\"$fragment\":{}}\n"}),
    ) {
        Outcome::Failed { message } => {
            assert!(message.starts_with("Not pasted: fragment line 2"), "{message}");
            assert_eq!(shell.banner(), Some(message.as_str()));
        }
        other => panic!("{other:?}"),
    }
    done(&mut shell, "select.set", json!({"ids": []}));
    assert_eq!(
        refused(&mut shell, "edit.copy", json!({})),
        "reason-nothing-selected"
    );
    assert_eq!(
        refused(&mut shell, "edit.cut", json!({})),
        "reason-nothing-selected"
    );
    assert_eq!(
        refused(&mut shell, "edit.duplicate", json!({})),
        "reason-nothing-selected"
    );
    // Copy tag list (FR-UI-040): -101F1, S51 and F55.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:S51", "tag:-101F1", "tag:F55"]}),
    );
    done(&mut shell, "edit.copy_tags", json!({}));
    assert_eq!(shell.clipboard().text(), Some("-101F1\nF55\nS51\n"));
    assert_eq!(state(&shell)["clipboard"], json!({"kind": "text", "lines": 3}));
    shell.shutdown();
}

/// CB8: the menu entries are disabled with their reason without a selection, and their chords are the canvas's own.
#[test]
fn cb8_menu_entries_and_chords() {
    use ew_viewmodel::actions::{Context, find};
    for (id, key) in [
        ("edit.cut", "Ctrl+X"),
        ("edit.copy", "Ctrl+C"),
        ("edit.paste_clip", "Ctrl+V"),
        ("edit.duplicate", "Ctrl+D"),
    ] {
        let a = find(id).unwrap();
        assert_eq!(a.shortcut, [key], "{id}");
        assert_eq!(a.context, Context::Canvas, "{id}");
    }
    assert!(find("clipboard.text").unwrap().headless_only);
    let tmp = Tmp::new("clip-menu");
    let mut shell = writable(&tmp, "101");
    let menus = ew_viewmodel::actions::menu_model(&shell);
    let edit = menus
        .iter()
        .find(|m| m.menu == ew_viewmodel::actions::Menu::Edit)
        .unwrap();
    let items: Vec<&ew_viewmodel::actions::ActionItem> = edit
        .entries
        .iter()
        .filter_map(|e| match e {
            ew_viewmodel::actions::MenuEntry::Item(i) => Some(i),
            _ => None,
        })
        .collect();
    for id in ["edit.cut", "edit.copy", "edit.duplicate", "edit.copy_tags"] {
        let item = items
            .iter()
            .find(|i| i.id == id)
            .unwrap_or_else(|| panic!("{id}"));
        assert!(!item.enabled, "{id} without a selection");
    }
    assert!(items.iter().find(|i| i.id == "edit.paste_clip").unwrap().enabled);
    shell.shutdown();
}

/// The structural words of a describe line (as `tests/part2.rs` takes them out).
fn structural_words(line: &str, shortcut_keys: &[String]) -> Vec<String> {
    let mut outside = String::new();
    let mut quoted = false;
    for c in line.chars() {
        if c == '"' {
            quoted = !quoted;
            outside.push(' ');
        } else if !quoted {
            outside.push(c);
        }
    }
    let outside = outside.replace(['(', ')', '[', ']', ',', '|', '%'], " ");
    let mut words = Vec::new();
    for token in outside.split_whitespace() {
        if ew_viewmodel::describe::FREE_TEXT_MARKERS.contains(&token) {
            words.push(token.to_owned());
            break;
        }
        let skip = token.chars().all(|c| c.is_ascii_digit())
            // An ID alias `@n` is a value token like a number (ID-630).
            || ew_viewmodel::alias::is_alias(token)
            || token.starts_with('#')
            || token.contains('.')
            || token.contains('+')
            || token.chars().all(|c| !c.is_alphanumeric())
            || shortcut_keys.iter().any(|k| k == token);
        if !skip {
            words.push(token.to_owned());
        }
    }
    words
}
