// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip follow-ups, headless: a terminal chosen with no strip goes on a new strip made for it,
//! the PE terminal asks for its strip, the page description lists a strip's bridges,
//! table mode adds an accessory before the first terminal, and Enter on an unplaced terminal row
//! arms the Place tool with its function (§6.8).

use crate::common;

use common::{Tmp, done, new_project, refused, state, strip_at, strip_row, toast};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::PickerMode;
use serde_json::json;

/// Opens the device browser filtered to `filter` and chooses its first row.
fn choose(s: &mut ShellModel, filter: &str) {
    done(s, "tool.place_part", json!({}));
    done(s, "picker.filter", json!({ "text": filter }));
    done(s, "picker.choose", json!({}));
}

// ---------------------------------------------------------------------------------------------------------------

/// A terminal chosen in the picker of a project without strips goes on a new strip, made for it: Place is armed with
/// the terminal on it (a persona run: the empty strip chooser that opened instead stood between Place and the page).
/// With a strip in the project the chooser lists it. A direct `place.pick` still refuses with the sentence that names
/// Insert ▸ Terminal strip.
#[test]
fn the_picker_puts_a_terminal_on_a_new_strip_when_the_project_has_none() {
    let tmp = Tmp::new("s207p2-605");
    let mut s = new_project(&tmp);
    assert_eq!(
        refused(&mut s, "place.pick", json!({"symbol": "ew.iec.terminal@qet10"})),
        "reason-no-strip"
    );
    choose(&mut s, "EW-GEN-TRM-1");
    assert!(s.picker().is_none(), "no strip chooser");
    assert_eq!(s.tool().name(), "place");
    assert_eq!(
        toast(&s),
        "The terminal goes on a new terminal strip: click where it is to stand."
    );
    let ghost = &state(&s)["canvas"]["ghost"];
    assert_eq!(ghost["target"], "terminal", "{ghost}");
    done(&mut s, "ui.dismiss", json!({}));
    choose(&mut s, "EW-GEN-TRM-1");
    let p = s.picker().expect("the strip chooser");
    assert_eq!(p.mode, PickerMode::Strip);
    assert_eq!(p.rows.len(), 1, "the strip made for the first terminal");
    done(&mut s, "picker.choose", json!({}));
    assert_eq!(s.tool().name(), "place");
    assert!(s.picker().is_none());
}

// ---------------------------------------------------------------------------------------------------------------

/// The PE terminal's symbol asks for its strip as the feed-through terminal does: picked without a strip it opens
/// the strip chooser, and the chosen strip's terminal is armed.
#[test]
fn the_pe_terminal_opens_the_strip_chooser() {
    let tmp = Tmp::new("s207p2-618");
    let mut s = new_project(&tmp);
    done(&mut s, "strip.new", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal-pe@qet10"}),
    );
    let p = s.picker().expect("the strip chooser");
    assert_eq!(p.mode, PickerMode::Strip);
    assert_eq!(p.rows.len(), 1);
    done(&mut s, "picker.choose", json!({ "designation": "PE" }));
    assert_eq!(s.tool().name(), "place");
    let ghost = &state(&s)["canvas"]["ghost"];
    assert_eq!(ghost["target"], "terminal", "{ghost}");
}

// ---------------------------------------------------------------------------------------------------------------

fn page_text(s: &mut ShellModel, folio: &str) -> String {
    done(s, "page.describe", json!({ "folio": folio }));
    s.take_page_text().expect("page.describe leaves its text")
}

/// The page description lists the bridges of the strips it draws, one `bridge` line per group with its terminals;
/// a page without a bridged strip lists none, and a new bridge shows on the next description.
#[test]
fn the_page_description_lists_a_strips_bridges() {
    let tmp = Tmp::new("s207p2-621");
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    let text = page_text(&mut s, "102");
    let bridges: Vec<&str> = text.lines().filter(|l| l.starts_with("  bridge ")).collect();
    assert!(
        bridges.contains(&"  bridge \"BR1\" \"-X1021:1\" \"-X1021:2\" \"-X1021:3\" \"-X1021:4\""),
        "{bridges:?}"
    );
    // Folio 102 draws -X1021 with the two groups the sample's script bridges (BR1 and BR2).
    assert_eq!(bridges.len(), 2, "{bridges:?}");
    assert!(
        bridges[1].starts_with("  bridge \"BR2\" \"-X1021:5\""),
        "{bridges:?}"
    );
    assert!(!page_text(&mut s, "101").contains("\n  bridge "));
    // Bridging two terminals of -X1011 shows on folio 101's description.
    done(&mut s, "select.set", json!({"refs": ["tag:-X1011:L1", "tag:-X1011:L2"]}));
    done(&mut s, "strip.bridge", json!({}));
    let text = page_text(&mut s, "101");
    let line = text
        .lines()
        .find(|l| l.starts_with("  bridge "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(line.ends_with("\"-X1011:L1\" \"-X1011:L2\""), "{line}");
    s.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// §6.8

/// A terminal row whose terminal has no representation on a strip without strip segments carries the `unplaced` badge
/// and is a drag source naming its terminal; Enter (`navigator.open`) on it arms the Place tool with the terminal's
/// main function on the page shown, and a click places it, so the badge goes. Terminals drawn by a strip segment
/// (-X1021) are not unplaced.
#[test]
fn enter_on_an_unplaced_terminal_row_arms_place_with_its_function() {
    let tmp = Tmp::new("s207p2-68");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "strip.edit", json!({"strip": "-X1011"}));
    // A new terminal at the end of -X1011 (it has no strip segment): unplaced.
    let last = strip_row(&s, "Terminal", "PE");
    strip_at(&mut s, &last);
    done(&mut s, "strip.insert", json!({"designation": "9"}));
    let new = strip_row(&s, "Terminal", "9");
    let rows = s.shown_rows(NavKind::Strips);
    let r = rows.iter().find(|r| r.key == new).expect("the new row");
    assert!(r.badges.iter().any(|b| b == "not drawn"), "{:?}", r.badges);
    assert!(matches!(
        &r.drag,
        Some(ew_viewmodel::shell::DragSource::Function(_))
    ));
    let placed = strip_row(&s, "Terminal", "L1");
    let r1 = rows.iter().find(|r| r.key == placed).unwrap();
    assert!(!r1.badges.iter().any(|b| b == "not drawn"));
    // Enter on the row (in the tree view) arms Place with the terminal's function.
    done(&mut s, "navigator.table", json!({"kind": "strips", "on": false}));
    done(&mut s, "navigator.open", json!({"kind": "strips", "key": new}));
    assert_eq!(s.tool().name(), "place");
    let ghost = &state(&s)["canvas"]["ghost"];
    assert_eq!(ghost["target"], "function", "{ghost}");
    // A click places it: the row loses the badge.
    let p = common::empty_point(s.canvas().unwrap());
    common::click(&mut s, p, json!({}));
    let rows = s.shown_rows(NavKind::Strips);
    let r = rows.iter().find(|r| r.key == new).expect("the row");
    assert!(!r.badges.iter().any(|b| b == "not drawn"), "{:?}", r.badges);
    // -X1021's terminals are drawn by its strip segments.
    done(
        &mut s,
        "navigator.filter",
        json!({"kind": "strips", "text": "strip:-x1021"}),
    );
    let rows = s.shown_rows(NavKind::Strips);
    assert!(rows.iter().all(|r| !r.badges.iter().any(|b| b == "not drawn")));
    s.shutdown();
}
