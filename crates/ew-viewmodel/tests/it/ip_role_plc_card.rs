// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cycle-39 play-test's findings FT-822 and FT-823 in the view model (cycle 40, lane G40).
//!
//! FT-822: the page-arrow tool offers the target role for a name whose source stands on another page (EW-CORE-052 is
//! project-wide), its HUD names the role and the key that changes it, D goes on from the offered role, and a refused
//! arrow keeps its ghost with the reason in the HUD and a toast. FT-823: a card drawn before the CPU is attached when
//! the CPU arrives (its channels get addresses), the page description lists the addresses the canvas prints, and
//! deleting the card's symbol leaves no `-?A1` device behind.

use crate::common;

use common::{Tmp, done, refused, state};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
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

/// The IP tool's ghost of the transcript state.
fn ghost(s: &ShellModel) -> Value {
    state(s)["canvas"]["ghost"].clone()
}

/// Arms the IP tool with the typed name `name` and moves the grid cursor into the page's right quarter (a source by
/// the quarters rule), at height `y` mm.
fn arm_ip(s: &mut ShellModel, name: &str, y: f64) {
    done(s, "tool.ip", json!({}));
    done(s, "ip.name", json!({"text": name}));
    done(s, "cursor.set", json!({"x_mm": 330.0, "y_mm": y}));
}

/// The project with a source point 0V on page 101; page 102 shown.
fn source_on_101(tmp: &Tmp) -> ShellModel {
    let mut s = new_project(tmp);
    arm_ip(&mut s, "0V", 150.0);
    assert_eq!(
        ghost(&s)["direction"],
        "source",
        "no 0V source yet: the quarter gives source"
    );
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    s
}

#[test]
fn ft822_a_name_with_a_source_on_another_page_is_offered_as_a_target() {
    let tmp = Tmp::new("g40-offer");
    let mut s = source_on_101(&tmp);
    arm_ip(&mut s, "0V", 150.0);
    let g = ghost(&s);
    assert_eq!(
        (g["name"].as_str(), g["direction"].as_str()),
        (Some("0V"), Some("target"))
    );
    // The target is placed: no refusal, and it pairs with the source on 101.
    done(&mut s, "canvas.activate", json!({}));
    let t = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
    let ip = t
        .reps
        .iter()
        .find_map(|r| r.ip.clone())
        .expect("the placed point");
    assert_eq!((ip.name.as_str(), ip.direction.as_str()), ("0V", "target"));
}

#[test]
fn ft822_on_the_page_of_the_source_the_series_goes_on_as_before() {
    let tmp = Tmp::new("g40-series");
    let mut s = new_project(&tmp);
    arm_ip(&mut s, "24VDC-1", 150.0);
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "cursor.set", json!({"x_mm": 330.0, "y_mm": 170.0}));
    let g = ghost(&s);
    assert_eq!(
        (g["name"].as_str(), g["direction"].as_str()),
        (Some("24VDC-2"), Some("source")),
        "the source stands on this page: the next source of the series"
    );
}

#[test]
fn ft822_the_hud_names_the_role_and_d_goes_on_from_the_offered_one() {
    let tmp = Tmp::new("g40-hud");
    let mut s = source_on_101(&tmp);
    arm_ip(&mut s, "0V", 150.0);
    let prompt = ew_viewmodel::command_bar::hud(&s).prompt;
    assert!(prompt.contains("target"), "{prompt}");
    assert!(
        prompt.contains(": undirected"),
        "the key and the next role: {prompt}"
    );
    // D goes on from the offered target, not from the quarter's source.
    done(&mut s, "tool.ip_direction", json!({}));
    assert_eq!(ghost(&s)["direction"], "undirected");
    done(&mut s, "tool.ip_direction", json!({}));
    let g = ghost(&s);
    assert_eq!(
        (g["name"].as_str(), g["direction"].as_str()),
        (Some("0V"), Some("source")),
        "a typed name stays the user's"
    );
}

#[test]
fn ft822_choosing_source_for_a_proposed_name_offers_the_next_free_name() {
    let tmp = Tmp::new("g40-free");
    let mut s = source_on_101(&tmp);
    // No typed name: the proposal is the last committed name, 0V.
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "cursor.set", json!({"x_mm": 330.0, "y_mm": 150.0}));
    assert_eq!(ghost(&s)["name"], "0V");
    assert_eq!(ghost(&s)["direction"], "target");
    done(&mut s, "tool.ip_direction", json!({"direction": "source"}));
    let g = ghost(&s);
    assert_eq!(
        (g["name"].as_str(), g["direction"].as_str()),
        (Some("0V-1"), Some("source")),
        "0V has a source: the series' next name that has none"
    );
    done(&mut s, "canvas.activate", json!({}));
}

#[test]
fn ft822_a_refused_arrow_keeps_its_ghost_and_says_why() {
    let tmp = Tmp::new("g40-refused");
    let mut s = source_on_101(&tmp);
    arm_ip(&mut s, "0V", 150.0);
    done(&mut s, "tool.ip_direction", json!({"direction": "source"}));
    let items = s.tool_overlay().items.len();
    assert!(items > 0, "the ghost is drawn");
    assert_eq!(
        refused(&mut s, "canvas.activate", json!({})),
        "reason-ip-not-placed"
    );
    assert_eq!(
        s.tool_overlay().items.len(),
        items,
        "the ghost stays where it was"
    );
    assert_eq!(s.tool().name(), "ip", "the tool stays armed");
    // H40 item 6: what is in the way and the way out, in words: not the rule's ID tie-break.
    let words = "Not placed: 0V already has a source on page 101. Press D for a target, or mark one source as the star.";
    let notice = s.tool_notice().expect("the reason in the HUD").to_owned();
    assert_eq!(notice, words);
    assert!(
        ew_viewmodel::command_bar::hud(&s).prompt.contains(words),
        "the HUD shows the refusal"
    );
    let toasts = s.notifications().visible();
    assert!(
        toasts.iter().any(|n| n.text == words),
        "a warning toast says it too"
    );
    assert_eq!(state(&s)["banner"], Value::Null, "no banner for a refused click");
}

/// Draws the black box of the starter part `part` from cell `a` to cell `b` of page 102 and ends the tool.
fn black_box(s: &mut ShellModel, part: &str, a: &str, b: &str) {
    done(s, "tool.black_box", json!({"part": part}));
    done(s, "nav.goto", json!({"text": a}));
    done(s, "canvas.activate", json!({}));
    done(s, "nav.goto", json!({"text": b}));
    done(s, "canvas.activate", json!({}));
    done(s, "ui.dismiss", json!({}));
    done(s, "ui.dismiss", json!({}));
}

/// Page 102 with the digital input card drawn before the CPU.
fn card_before_cpu(tmp: &Tmp) -> ShellModel {
    let mut s = new_project(tmp);
    done(&mut s, "page.goto", json!({"folio": "102"}));
    black_box(&mut s, "EW-GEN-DI-1", "102-D5", "102-E10");
    black_box(&mut s, "EW-GEN-CPU-1", "102-F5", "102-G9");
    s
}

#[test]
fn ft823_a_card_drawn_before_the_cpu_is_addressed_when_the_cpu_arrives() {
    let tmp = Tmp::new("g40-card");
    let s = card_before_cpu(&tmp);
    let t = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
    let addressed: Vec<(String, String)> = t
        .cps
        .iter()
        .filter_map(|c| Some((c.label.clone(), c.plc_address.clone()?)))
        .collect();
    assert!(
        addressed.contains(&("-102A1:1".to_owned(), "I0.0".to_owned())),
        "the card's first input is I0.0: {addressed:?}"
    );
}

#[test]
fn ft823_the_page_description_lists_the_addresses_the_canvas_prints() {
    let tmp = Tmp::new("g40-describe");
    let mut s = card_before_cpu(&tmp);
    done(&mut s, "page.describe", json!({}));
    let text = s.take_page_text().expect("the page text");
    let card = text
        .lines()
        .find(|l| l.starts_with("  rep \"-102A1\""))
        .unwrap_or_else(|| panic!("the card's line: {text}"));
    assert!(card.contains(" address \"1=I0.0\" \"2=I0.1\""), "{card}");
    // The connection-point lines keep their form.
    assert!(
        text.lines()
            .filter(|l| l.starts_with("    cp "))
            .all(|l| !l.contains("address")),
        "{text}"
    );
}

#[test]
fn ft823_deleting_the_cards_symbol_leaves_no_device_behind() {
    let tmp = Tmp::new("g40-orphan");
    let mut s = card_before_cpu(&tmp);
    done(&mut s, "select.set", json!({"refs": ["tag:-102A1"]}));
    assert_eq!(s.dispatch("edit.delete_selection", &json!({})), Outcome::Done);
    done(&mut s, "navigator.show", json!({"kind": "devices"}));
    let labels: Vec<String> = s
        .shown_rows(NavKind::Devices)
        .into_iter()
        .map(|r| r.label)
        .collect();
    assert!(
        labels.iter().all(|l| !l.contains("A1") || l.contains("A2")),
        "no -?A1 left: {labels:?}"
    );
    assert!(labels.iter().any(|l| l == "-102A2"), "the CPU stays: {labels:?}");
    // One undo brings the card back with its symbol.
    done(&mut s, "edit.undo", json!({}));
    let t = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
    assert!(t.cps.iter().any(|c| c.label == "-102A1:1"));
}
