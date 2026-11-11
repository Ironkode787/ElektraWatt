// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's dead ends of M2.5-Fix's section "The window: feedback and dead ends", each by the pointer and the
//! keys: a panel found by name in the command search, a PLC card that waits for a CPU, a device selected by its tag
//! outlined, Ctrl+A on the page, a page's Pending badge that counts what the status bar counts, and references and
//! functions named in words in Properties and Choose part.

use crate::support;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input, parse_chord};
use serde_json::json;
use support::Tmp;

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn key(chord: &str) -> Input {
    let (key, mods) = parse_chord(chord).unwrap();
    Input::Key { key, mods, repeat: 1 }
}

fn typed(text: &str) -> Input {
    Input::Type {
        text: text.to_owned(),
    }
}

/// A driver on a new project from the built-in template, with its accessibility tree.
fn new_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// A driver over a copy of Golden A on folio 101.
fn golden_on_101(tmp: &Tmp) -> Driver {
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    d
}

/// Whether the left rail shows the panel of `word`.
fn panel_shown(d: &mut Driver, word: &str) -> bool {
    ew_ui::ew_viewmodel::rail::rail(d.shell_mut())
        .left
        .iter()
        .any(|b| b.word == word && b.active)
}

/// FT-927: the command search found nothing for "PLC" (the panel was behind its icon and View ▸ Navigators). It lists
/// the panel by its name; Enter shows it, and a second time leaves it shown.
#[test]
fn the_command_search_finds_the_plc_panel_by_its_name() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    assert!(!panel_shown(&mut d, "plc"));
    for _ in 0..2 {
        d.input(&key("ctrl+k"));
        d.input(&typed("PLC"));
        d.input(&key("enter"));
        d.settle();
        assert!(panel_shown(&mut d, "plc"), "the PLC panel is shown");
    }
    d.shell_mut().shutdown();
}

/// Places the part `number` from Draw ▸ Device as a box from `from` to `to`.
fn place_box(d: &mut Driver, number: &str, from: (i64, i64), to: (i64, i64)) {
    d.input(&click(d.find("Device").unwrap()));
    d.input(&typed(number));
    d.input(&key("enter"));
    for at in [from, to] {
        d.input(&Input::Hover { at });
        d.input(&click(at));
    }
    d.input(&key("escape"));
    d.settle();
}

/// FT-926: a PLC card drawn before a CPU left the PLC panel reading "No PLC cards yet". It says that the card waits
/// for a CPU and how to place one; once a CPU is placed the card is listed under it.
#[test]
fn a_plc_card_without_a_cpu_says_in_the_plc_panel_that_it_waits_for_one() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_box(&mut d, "EW-GEN-DI-1", (700, 400), (900, 600));
    d.input(&click(d.find("PLC").unwrap()));
    d.settle();
    assert!(panel_shown(&mut d, "plc"));
    assert!(d.find("No CPU yet").is_some(), "{:?}", d.labels());
    assert!(d.find("No PLC cards yet").is_none());
    place_box(&mut d, "EW-GEN-CPU-1", (1000, 400), (1200, 600));
    assert!(d.find("No CPU yet").is_none(), "{:?}", d.labels());
    d.shell_mut().shutdown();
}

/// FT-1585: a device selected by its tag in the command search was counted in the status bar and drawn without an
/// outline. Its representations on the page are outlined.
#[test]
fn a_device_selected_by_its_tag_is_outlined_on_the_page() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    d.input(&key("ctrl+k"));
    d.input(&typed("F55"));
    d.input(&key("enter"));
    d.settle();
    let s = d.shell_mut();
    assert!(!s.selection().is_empty(), "something is selected");
    assert!(!s.selected_rects(0).is_empty(), "the device is outlined");
    d.shell_mut().shutdown();
}

/// FT-1586: Ctrl+A on the page selected nothing. It selects every object the page draws.
#[test]
fn ctrl_a_on_the_page_selects_everything_it_draws() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    // The canvas has the keys after a click on an empty spot of the sheet (below the drawing, above the title block).
    let empty = d.find("Zoom out").map(|(x, y)| (x - 200, y)).unwrap();
    d.input(&click(empty));
    d.input(&key("ctrl+a"));
    d.settle();
    let s = d.shell_mut();
    let mut drawn: Vec<_> = s.canvas().unwrap().page().entities.iter().map(|(e, _)| *e).collect();
    drawn.sort();
    drawn.dedup();
    let mut selected = s.selection().items.clone();
    selected.sort();
    assert_eq!(selected, drawn);
    d.shell_mut().shutdown();
}

/// The count of the first label that starts with a number and ends with `word` ("9 Pending", "9 pending on this
/// page").
fn count_before(d: &Driver, word: &str) -> Option<usize> {
    d.labels().into_iter().find_map(|(l, _)| {
        let (n, rest) = l.split_once(' ')?;
        rest.starts_with(word).then(|| n.parse().ok()).flatten()
    })
}

/// FT-1587: a page's Properties badge read fewer Pending findings than the status bar counted on the page: a finding
/// of a device on the page (a part missing) has no page of its own. Both count it.
#[test]
fn a_pages_pending_badge_counts_what_the_status_bar_counts() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    for (name, at) in [("fuse", (600, 400)), ("lamp", (900, 400))] {
        d.input(&click(d.find("Symbol").unwrap()));
        d.input(&typed(name));
        d.input(&key("enter"));
        d.input(&Input::Hover { at });
        d.input(&click(at));
        d.input(&key("escape"));
        d.input(&key("escape"));
        d.settle();
    }
    // Nothing selected: Properties shows the page; with Messages open the status bar counts the page's pending
    // findings.
    d.input(&click((1100, 800)));
    let _ = d.shell_mut().dispatch("messages.show", &json!({"on": true}));
    d.input(&Input::Wait { ms: 300 });
    d.settle();
    let bar = count_before(&d, "pending on this page")
        .unwrap_or_else(|| panic!("the status bar's count: {:?}", d.labels()));
    assert!(bar > 0);
    assert_eq!(count_before(&d, "Pending"), Some(bar), "{:?}", d.labels());
    d.shell_mut().shutdown();
}

/// Whether `text` holds a UUID's text.
fn has_uuid(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_hexdigit() && c != '-')
        .any(|w| w.len() == 36 && w.matches('-').count() == 4)
}

/// FT-430: Properties printed a representation's symbol and its function's definition as the copies' UUIDs. A click
/// on a symbol shows both in words.
#[test]
fn a_symbols_properties_name_its_symbol_and_definition_in_words() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    // The window's pixels of a sheet point, from two pointer positions the canvas reports.
    let probe = |d: &mut Driver, at: (i64, i64)| {
        d.input(&Input::Hover { at });
        d.settle();
        let p = state(d)["canvas"]["pointer_nm"].clone();
        (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())
    };
    let (f0, f1) = ((700, 400), (1000, 700));
    let (s0, s1) = (probe(&mut d, f0), probe(&mut d, f1));
    let pixel = |sheet: (i64, i64)| {
        (
            f0.0 + (sheet.0 - s0.0) * (f1.0 - f0.0) / (s1.0 - s0.0),
            f0.1 + (sheet.1 - s0.1) * (f1.1 - f0.1) / (s1.1 - s0.1),
        )
    };
    // F55's symbol: the outline its device draws, found by the tag.
    let _ = d.shell_mut().dispatch("select.set", &json!({"refs": ["tag:F55"]}));
    d.settle();
    let rect = d.shell_mut().selected_rects(0)[0];
    let _ = d.shell_mut().dispatch("select.set", &json!({"refs": []}));
    d.input(&click(pixel(ew_ui::ew_viewmodel::canvas::rect_centre(rect))));
    d.settle();
    let text = ew_ui::ew_viewmodel::describe::describe(d.shell_mut());
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("row \"representation/symbol\"") || l.contains("row \"function/def\""))
        .collect();
    assert_eq!(rows.len(), 2, "{text}");
    assert!(rows.iter().all(|l| !has_uuid(l)), "{rows:?}");
    d.shell_mut().shutdown();
}

fn state(d: &mut Driver) -> serde_json::Value {
    serde_json::from_str(&ew_ui::ew_viewmodel::script::state_json(d.shell_mut())).unwrap()
}

/// FT-819: Choose part's detail line printed the functions' keys ("functions ew.fn.mcb-1pn"). It names them in words.
#[test]
fn choose_part_names_a_parts_functions_in_words() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    let _ = d.shell_mut().dispatch("select.set", &json!({"refs": ["tag:F55"]}));
    d.settle();
    assert_eq!(
        d.shell_mut().dispatch("part.choose", &json!({"use": 0})),
        Outcome::Done
    );
    d.settle();
    let details: Vec<String> = (0..20)
        .filter_map(|i| d.shell_mut().part_offer_detail(i))
        .filter(|l| l.contains("·"))
        .collect();
    assert!(!details.is_empty());
    assert!(
        details.iter().all(|l| !l.contains("ew.fn.") && !has_uuid(l)),
        "{details:?}"
    );
    d.shell_mut().shutdown();
}
