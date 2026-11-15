// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A symbol's text moved on its own on a schematic page (doc 06 §2.5; a known limit of the first 0.3.0 candidate: a
//! text could not be moved without its symbol), through the shell on a temporary copy of Golden A: with the symbol
//! selected its tag drags alone on the fine grid, the Rotate key turns it while it is dragged, Undo takes the move
//! back, its menu hides a text, and Reset text positions puts the texts back.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::ew_api::output::SlotTextBox;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

const MM: i64 = 1_000_000;

/// The texts of the one selected symbol where the page prints them.
fn texts(shell: &ShellModel) -> Vec<SlotTextBox> {
    shell.selected_rep_texts().expect("one symbol selected").1
}

/// The sheet point of the middle of `t`'s box.
fn middle(shell: &ShellModel, t: &SlotTextBox) -> (i64, i64) {
    let r = t.rect;
    shell
        .canvas()
        .unwrap()
        .page()
        .snapper
        .page_to_sheet(((r.min().x.0 + r.max().x.0) / 2, (r.min().y.0 + r.max().y.0) / 2))
}

/// The selected symbol's text of slot `slot`.
fn slot(shell: &ShellModel, slot: u16) -> Option<SlotTextBox> {
    texts(shell).into_iter().find(|t| t.slot == slot)
}

#[test]
fn a_selected_symbols_text_drags_alone_turns_hides_and_goes_back() {
    let tmp = Tmp::new("symbol-text-drag");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    // A symbol of a device that prints texts, selected by a click on its body.
    let page = shell.canvas().unwrap().page().clone();
    let rep = page
        .tables
        .reps
        .iter()
        .find(|r| r.device.is_some() && r.black_box.is_none() && page.texts.iter().any(|t| t.rep == r.id))
        .expect("a symbol with texts")
        .clone();
    let body = page.snapper.page_to_sheet((
        (rep.body[0] + rep.body[2]) / 2,
        (rep.body[1] + rep.body[3]) / 2,
    ));
    done(&mut shell, "canvas.press", at(body, json!({})));
    done(&mut shell, "canvas.release", at(body, json!({})));
    let before = texts(&shell);
    let tag = *before.first().expect("the symbol prints a text");
    let grab = middle(&shell, &tag);
    // The drag: 10 mm right and 5 mm down, the Rotate key once on the way.
    let to = (grab.0 + 10 * MM, grab.1 + 5 * MM);
    done(&mut shell, "canvas.press", at(grab, json!({})));
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    assert_eq!(shell.tool().state(), "dragging");
    done(&mut shell, "tool.rotate_cw", json!({}));
    done(&mut shell, "canvas.release", at(to, json!({})));
    let moved = slot(&shell, tag.slot).expect("the text still prints");
    assert!(moved.moved, "an override moves it");
    assert!(moved.at.x.0 > tag.at.x.0 + 8 * MM, "{:?} → {:?}", tag.at, moved.at);
    assert_ne!(moved.angle, tag.angle, "the Rotate key turned it");
    // Its anchor lies on the fine grid of the page (a QET grid page: 793 750 / 3 nm, each point rounded to the nm).
    let off = (i128::from(moved.at.x.0) * 3).rem_euclid(793_750);
    assert!(off <= 3 || off >= 793_750 - 3, "{:?}", moved.at);
    // The symbol stays where it was: its other texts did not move.
    for t in before.iter().filter(|t| t.slot != tag.slot) {
        assert_eq!(slot(&shell, t.slot).map(|n| n.at), Some(t.at), "slot {}", t.slot);
    }
    // Undo takes it back as one step.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(slot(&shell, tag.slot).map(|t| (t.at, t.moved)), Some((tag.at, false)));
    done(&mut shell, "edit.redo", json!({}));
    // The menu over the text hides it; Reset text positions puts the moved one back.
    let over = middle(&shell, &slot(&shell, tag.slot).unwrap());
    done(&mut shell, "canvas.pointer", at(over, json!({})));
    let menu = canvas_menu(&shell, MenuSpot::Object);
    let hide = menu
        .iter()
        .find(|i| i.id == "rep.hide_text")
        .expect("Hide text over the text");
    assert_eq!(hide.params, json!({"slot": tag.slot}));
    assert!(menu.iter().any(|i| i.id == "rep.reset_texts"), "Reset text positions");
    done(&mut shell, "rep.reset_texts", json!({}));
    assert_eq!(slot(&shell, tag.slot).map(|t| (t.at, t.moved)), Some((tag.at, false)));
    done(&mut shell, "rep.hide_text", json!({"slot": tag.slot}));
    assert!(slot(&shell, tag.slot).is_none(), "the hidden text prints no more");
    shell.shutdown();
}
