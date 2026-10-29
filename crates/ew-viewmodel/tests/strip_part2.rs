// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip follow-ups of WP-S2.07 part 2, headless: a terminal chosen with no strip opens the strip chooser on its
//! empty state (FT-605), the PE terminal asks for its strip (FT-618), the page description lists a strip's bridges
//! (FT-621), table mode adds an accessory before the first terminal (FT-600), and Enter on an unplaced terminal row
//! arms the Place tool with its function (§6.8).

mod common;

use common::{Tmp, done, refused, state};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::tools::PickerMode;
use serde_json::json;

/// A headless shell with a new project (the builtin template) and a new folio 101 shown.
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
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

/// Opens the picker filtered to `filter` and chooses its first row.
fn choose(s: &mut ShellModel, filter: &str) {
    done(s, "tool.place", json!({}));
    done(s, "picker.filter", json!({ "text": filter }));
    done(s, "picker.choose", json!({}));
}

// ---------------------------------------------------------------------------------------------------------------
// FT-605

/// A terminal chosen in the picker of a project without strips opens the strip chooser on its empty state (the
/// window's empty state says why and offers New terminal strip); `strip.new` lists the new strip as the current row
/// and choosing it arms Place. A direct `place.pick` still refuses with the sentence that names Insert ▸ Terminal
/// strip.
#[test]
fn ft605_the_picker_opens_the_empty_strip_chooser_and_strip_new_fills_it() {
    let tmp = Tmp::new("s207p2-605");
    let mut s = new_project(&tmp);
    assert_eq!(
        refused(&mut s, "place.pick", json!({"symbol": "ew.iec.terminal@qet10"})),
        "reason-no-strip"
    );
    choose(&mut s, "ew.iec.terminal@");
    let p = s.picker().expect("the strip chooser");
    assert_eq!(p.mode, PickerMode::Strip);
    assert!(p.rows.is_empty());
    assert_eq!(p.empty_text(), Some("picker-empty-strips"));
    done(&mut s, "strip.new", json!({}));
    let p = s.picker().expect("still open");
    assert_eq!(p.mode, PickerMode::Strip);
    assert_eq!(p.rows.len(), 1);
    done(&mut s, "picker.choose", json!({}));
    assert_eq!(s.tool().name(), "place");
    assert!(s.picker().is_none());
}

// ---------------------------------------------------------------------------------------------------------------
// FT-618

/// The PE terminal's symbol asks for its strip as the feed-through terminal does: picked without a strip it opens
/// the strip chooser, and the chosen strip's terminal is armed.
#[test]
fn ft618_the_pe_terminal_opens_the_strip_chooser() {
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
// FT-621

fn page_text(s: &mut ShellModel, folio: &str) -> String {
    done(s, "page.describe", json!({ "folio": folio }));
    s.take_page_text().expect("page.describe leaves its text")
}

/// The page description lists the bridges of the strips it draws, one `bridge` line per group with its terminals;
/// a page without a bridged strip lists none, and a new bridge shows on the next description.
#[test]
fn ft621_the_page_description_lists_a_strips_bridges() {
    let tmp = Tmp::new("s207p2-621");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    let text = page_text(&mut s, "103");
    let bridges: Vec<&str> = text.lines().filter(|l| l.starts_with("  bridge ")).collect();
    assert!(
        bridges.contains(&"  bridge \"BR1\" \"-X1031:1\" \"-X1031:2\" \"-X1031:3\" \"-X1031:4\""),
        "{bridges:?}"
    );
    // Folio 103 draws -X1031 (BR1 to BR5) and -X1032 (BR6).
    assert_eq!(bridges.len(), 6, "{bridges:?}");
    assert!(
        bridges[5].starts_with("  bridge \"BR6\" \"-X1032:1\""),
        "{bridges:?}"
    );
    assert!(!page_text(&mut s, "101").contains("\n  bridge "));
    // Bridging two terminals of 1X1 shows on folio 101's description.
    done(&mut s, "select.set", json!({"refs": ["tag:1X1:1", "tag:1X1:2"]}));
    done(&mut s, "strip.bridge", json!({}));
    let text = page_text(&mut s, "101");
    let line = text
        .lines()
        .find(|l| l.starts_with("  bridge "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(line.ends_with("\"1X1:1\" \"1X1:2\""), "{line}");
    s.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// Strip editor helpers (as in strip_editor.rs)

/// The texts of the Terminal strips column labelled `label` on show, with the row keys.
fn column(s: &ShellModel, label: &str) -> Vec<(String, String)> {
    let n = s.nav(NavKind::Strips).unwrap();
    let t = n.table().unwrap();
    let c = t.columns.iter().position(|c| c.label == label).unwrap();
    n.visible()
        .iter()
        .map(|r| {
            let node = &t.nodes[r.node as usize];
            (node.key.clone(), node.cells[c].clone())
        })
        .collect()
}

fn texts(s: &ShellModel, label: &str) -> Vec<String> {
    column(s, label).into_iter().map(|(_, t)| t).collect()
}

fn row(s: &ShellModel, label: &str, text: &str) -> String {
    column(s, label)
        .into_iter()
        .find(|(_, t)| t == text)
        .unwrap_or_else(|| panic!("no row with {label} {text}"))
        .0
}

fn cursor(s: &ShellModel) -> String {
    s.nav(NavKind::Strips)
        .and_then(|n| n.cursor_key())
        .expect("a cursor row")
}

fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

/// A writable copy of Golden A, headless, with the strip editor open on `strip` (table mode, filtered).
fn editor(tmp: &Tmp, strip: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "strip.edit", json!({ "strip": strip }));
    s
}

fn at(s: &mut ShellModel, key: &str) {
    done(s, "table.cell", json!({"kind": "strips", "row": key}));
}

// ---------------------------------------------------------------------------------------------------------------
// FT-600

/// Table mode reaches the start of a strip: Add accessory with `before` on the first terminal row adds the accessory
/// at `after_seq` 0 (Pos 0.1), on a later terminal directly before it (after the terminal before it and the accessories
/// already there); one undo step each. Without `before` it goes after the row as built.
#[test]
fn ft600_add_accessory_before_the_first_terminal_in_table_mode() {
    let tmp = Tmp::new("s207p2-600");
    let mut s = editor(&tmp, "-X1031");
    let k1 = row(&s, "Terminal", "1");
    assert_eq!(cursor(&s), k1);
    let steps = s.recorded_commands().len();
    done(&mut s, "strip.add_accessory", json!({"before": true}));
    assert_eq!(s.recorded_commands().len(), steps + 1, "one command");
    assert_eq!(toast(&s), "Added End bracket before -X1031:1");
    assert_eq!(texts(&s, "Pos")[..2], ["0.1", "1"]);
    assert_eq!(cursor(&s), row(&s, "Pos", "0.1"));
    // Before terminal 5: after terminal 4.
    let k5 = row(&s, "Terminal", "5");
    at(&mut s, &k5);
    done(&mut s, "strip.add_accessory", json!({"before": true}));
    let pos = texts(&s, "Pos");
    let i = pos.iter().position(|p| p == "4.1").expect("4.1");
    assert_eq!(pos[i - 1..=i + 1], ["4", "4.1", "5"]);
    // An undo removes the last one only.
    done(&mut s, "edit.undo", json!({}));
    assert!(!texts(&s, "Pos").contains(&"4.1".to_owned()));
    assert_eq!(texts(&s, "Pos")[0], "0.1");
    s.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// §6.8

/// A terminal row whose terminal has no representation on a strip without strip segments carries the `unplaced` badge
/// and is a drag source naming its terminal; Enter (`navigator.open`) on it arms the Place tool with the terminal's
/// main function on the page shown, and a click places it, so the badge goes. Terminals drawn by a strip segment
/// (-X1031) are not unplaced.
#[test]
fn s68_enter_on_an_unplaced_terminal_row_arms_place_with_its_function() {
    let tmp = Tmp::new("s207p2-68");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "strip.edit", json!({"strip": "1X1"}));
    // A new terminal at the end of 1X1 (it has no strip segment): unplaced.
    let last = row(&s, "Terminal", "PE");
    at(&mut s, &last);
    done(&mut s, "strip.insert", json!({"designation": "9"}));
    let new = row(&s, "Terminal", "9");
    let rows = s.shown_rows(NavKind::Strips);
    let r = rows.iter().find(|r| r.key == new).expect("the new row");
    assert!(r.badges.iter().any(|b| b == "unplaced"), "{:?}", r.badges);
    assert!(matches!(
        &r.drag,
        Some(ew_viewmodel::shell::DragSource::Function(_))
    ));
    let placed = row(&s, "Terminal", "1");
    let r1 = rows.iter().find(|r| r.key == placed).unwrap();
    assert!(!r1.badges.iter().any(|b| b == "unplaced"));
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
    assert!(!r.badges.iter().any(|b| b == "unplaced"), "{:?}", r.badges);
    // -X1031's terminals are drawn by its strip segments.
    done(
        &mut s,
        "navigator.filter",
        json!({"kind": "strips", "text": "strip:-x1031"}),
    );
    let rows = s.shown_rows(NavKind::Strips);
    assert!(rows.iter().all(|r| !r.badges.iter().any(|b| b == "unplaced")));
    s.shutdown();
}
