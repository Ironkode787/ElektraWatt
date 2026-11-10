// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The play-test polish of cycle 45 in the view model: FT-1023 (a renamed interruption point says so) and
//! FT-1032 (a stale refusal leaves the canvas hint). FT-1028's test is `h40.rs`'s.

use crate::common;

use common::{Tmp, done, state};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a new project (the builtin template) and the pages 101 and 102; 101 shown.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    for folio in ["102", "101"] {
        done(
            &mut s,
            "page.new",
            json!({"folio": folio, "type": "multi_line", "title": "Page"}),
        );
    }
    done(&mut s, "page.goto", json!({"folio": "101"}));
    s
}

fn ghost(s: &ShellModel) -> Value {
    state(s)["canvas"]["ghost"].clone()
}

fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

fn hud(s: &ShellModel) -> String {
    ew_viewmodel::command_bar::hud(s).prompt
}

/// The first shortcut of action `id` as the window's chips show it.
fn key_of(s: &ShellModel, id: &str) -> String {
    ew_viewmodel::keymap::display(&s.shortcut_text(ew_viewmodel::actions::find(id).unwrap()))
}

/// "when the tool proposes a name other than the potential's, the HUD and the placement toast read
/// '{base} is taken, so this point is {name}.'" A second source point after 0V is proposed as 0V-1: the HUD says so
/// in the name's place, and so does the toast of its placement.
#[test]
fn ft1023_a_renamed_point_says_its_base_is_taken_in_the_hud_and_the_toast() {
    let tmp = Tmp::new("p45b-taken");
    let mut s = new_project(&tmp);
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.name", json!({"text": "0V"}));
    done(&mut s, "cursor.set", json!({"x_mm": 330.0, "y_mm": 150.0}));
    // A typed name is the user's: no sentence.
    let d = key_of(&s, "tool.ip_direction");
    assert_eq!(
        hud(&s),
        format!("0V \u{b7} source: the wire continues at its targets \u{b7} {d}: target")
    );
    done(&mut s, "canvas.activate", json!({}));
    // The next proposal on the same page goes on from 0V, which has its source here.
    done(&mut s, "cursor.set", json!({"x_mm": 330.0, "y_mm": 170.0}));
    let g = ghost(&s);
    assert_eq!(
        (g["name"].as_str(), g["direction"].as_str()),
        (Some("0V-1"), Some("source"))
    );
    assert_eq!(
        hud(&s),
        format!(
            "0V is taken, so this point is 0V-1. \u{b7} source: the wire continues at its targets \u{b7} {d}: target"
        )
    );
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(toast(&s), "0V is taken, so this point is 0V-1.");
}

/// The IP tool's refusal of a point dropped on the interruption point Q9, as the canvas hint shows it.
const ON_IP: &str =
    "Not placed: the interruption point Q9 is already here; place it on a pin or a free grid point";

/// Places the interruption point Q9 at the grid cursor, then drops another on it: the refusal stands in the hint.
fn refuse_ip_on_ip(s: &mut ShellModel) {
    done(s, "tool.ip", json!({}));
    done(s, "ip.name", json!({"text": "Q9"}));
    done(s, "cursor.set", json!({"x_mm": 200.0, "y_mm": 40.0}));
    done(s, "canvas.activate", json!({}));
    assert_eq!(
        common::refused(s, "canvas.activate", json!({})),
        "reason-ip-not-placed"
    );
    assert_eq!(s.tool_notice(), Some(ON_IP));
}

/// A headless shell on a writable copy of Golden A, folio 101, with -101F1 selected and the IP tool's refusal in the
/// canvas hint.
fn refused(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "select.set", json!({"refs": ["tag:-101F1"]}));
    refuse_ip_on_ip(&mut s);
    // An action that changes neither the project nor the selection leaves it.
    done(&mut s, "view.zoom_actual", json!({}));
    assert_eq!(s.tool_notice(), Some(ON_IP));
    s
}

/// "a tool's refusal notice clears when the state it refused changes: a committed command".
#[test]
fn ft1032_a_committed_command_clears_the_refusal() {
    let tmp = Tmp::new("p45b-notice-commit");
    let mut s = refused(&tmp);
    done(&mut s, "strip.new", json!({"letter": "X"}));
    assert_eq!(s.tool_notice(), None);
}

/// "... a Properties edit".
#[test]
fn ft1032_a_properties_edit_clears_the_refusal() {
    let tmp = Tmp::new("p45b-notice-props");
    let mut s = refused(&tmp);
    done(
        &mut s,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    assert_eq!(s.tool_notice(), None);
}

/// "... a selection change".
#[test]
fn ft1032_a_selection_change_clears_the_refusal() {
    let tmp = Tmp::new("p45b-notice-select");
    let mut s = refused(&tmp);
    done(&mut s, "select.set", json!({"refs": ["tag:-102F1"]}));
    assert_eq!(s.tool_notice(), None);
}

/// "... undo or redo".
#[test]
fn ft1032_undo_and_redo_clear_the_refusal() {
    let tmp = Tmp::new("p45b-notice-undo");
    let mut s = refused(&tmp);
    // A step to undo that keeps Q9: a new terminal strip.
    done(&mut s, "strip.new", json!({"letter": "X"}));
    let refuse_again = |s: &mut ShellModel| {
        assert_eq!(
            common::refused(s, "canvas.activate", json!({})),
            "reason-ip-not-placed"
        );
        assert_eq!(s.tool_notice(), Some(ON_IP));
    };
    refuse_again(&mut s);
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.tool_notice(), None, "undo");
    refuse_again(&mut s);
    done(&mut s, "edit.redo", json!({}));
    assert_eq!(s.tool_notice(), None, "redo");
}

/// ID-1139 (W45.2 F3): the IP tool armed with no pointer over the canvas leads its HUD with its proposed name, never
/// with an empty part (" · source: …").
#[test]
fn the_armed_ip_tools_hud_leads_with_its_name() {
    let tmp = Tmp::new("p45b-armed");
    let mut s = new_project(&tmp);
    done(&mut s, "tool.ip", json!({}));
    let d = key_of(&s, "tool.ip_direction");
    assert_eq!(
        hud(&s),
        format!("IP \u{b7} source: the wire continues at its targets \u{b7} {d}: target")
    );
}

/// A greyed command's reason, clicked, is a toast whose line under the title starts with a
/// capital ("Draw on a schematic page"); the toast's own text stays the reason.
#[test]
fn ft1075_a_greyed_commands_reason_reads_with_a_capital_under_its_title() {
    let tmp = Tmp::new("p45b-reason-capital");
    let mut s = new_project(&tmp);
    let reason = ew_viewmodel::strings::text(ew_viewmodel::key!("reason-page-type-no-symbols"));
    s.explain_disabled(reason.to_owned());
    let n = *s.notifications().visible().last().expect("the toast");
    assert_eq!(n.text, reason);
    let (title, line) = n.lines();
    assert_eq!(title, "This page type takes no symbols or wires");
    assert_eq!(line.as_deref(), Some("Draw on a schematic page"));
}
