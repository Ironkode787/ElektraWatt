// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A play-tester's terminals had no part: Terminal…'s form gave none, so Check project reported every terminal as a
//! device without a part (EW-CORE-041), and a terminal's menu had no Choose part…. Each test by the pointer and the
//! keys in the window at 1920 by 1080, on folio 103 of a copy of Golden A with a new strip: the form proposes the part
//! of its kind, a terminal's menu chooses the part of the selected terminals, a strip's row chooses it for its
//! terminals without one, and Add terminal carries it on. A terminal's EW-CORE-041 is read where the window shows it,
//! in the header of its Properties.

use crate::support;

use egui::{Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::output::Uuid;
use ew_ui::ew_viewmodel::navigators::NavKind;
use ew_ui::ew_viewmodel::shell::Dialog;
use serde_json::json;
use support::{Tmp, dispatch, screen, settle, wait_for_page};

/// The window at 1920 by 1080 on folio 103 of a copy of Golden A.
fn window(tmp: &Tmp) -> Harness<'static, App> {
    let mut h = support::on_golden(tmp, "103");
    h.set_size(egui::Vec2::new(1920.0, 1080.0));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

/// A click of `button` at `pos` with `modifiers`.
fn click_at(h: &mut Harness<'_, App>, pos: Pos2, button: PointerButton, modifiers: Modifiers) {
    h.event(egui::Event::PointerMoved(pos));
    h.run_ok();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        });
    }
    h.run_ok();
    settle(h);
}

/// The window point of the grid point `k` (grid steps from the sheet's corner): a free spot of folio 103 for `(60, y)`.
fn grid(h: &Harness<'_, App>, k: (i64, i64)) -> Pos2 {
    let page = std::sync::Arc::clone(h.state().shell().canvas().unwrap().page());
    let sn = &page.snapper;
    screen(
        h,
        sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1))),
    )
}

/// The value of field `name` of the open form and the words its list shows for it.
fn form_choice(h: &Harness<'_, App>, name: &str) -> (String, String) {
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => {
            let field = f.fields.iter().find(|x| x.name == name).expect("the field");
            let words = field
                .options
                .iter()
                .find(|(v, _)| *v == field.value)
                .map(|(_, w)| w.clone())
                .unwrap_or_default();
            (field.value.clone(), words)
        }
        other => panic!("the form: {other:?}"),
    }
}

/// Opens the form's list `list` and clicks its entry whose words start with `entry`.
fn choose(h: &mut Harness<'static, App>, list: &str, entry: &str) {
    h.get_by_role_and_label(egui::Role::ComboBox, list).click();
    settle(h);
    h.query_all_by_label_contains(entry)
        .rfind(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(entry)))
        .unwrap_or_else(|| panic!("{entry} in the list {list}"))
        .click();
    settle(h);
}

/// Terminal… on the Draw tab with a new strip: its form. Returns the new strip's tag, as its choice names it.
fn open_form(h: &mut Harness<'static, App>) -> String {
    dispatch(h, "view.command_tab", json!({ "tab": "draw" }));
    settle(h);
    support::bar_button(h, "Terminal").click();
    settle(h);
    choose(h, "Strip", "New strip");
    let (_, words) = form_choice(h, "strip");
    words
        .split_once('(')
        .and_then(|(_, t)| t.strip_suffix(')'))
        .unwrap_or_else(|| panic!("the new strip's tag in {words}"))
        .to_owned()
}

/// The page's terminal representations.
fn terminal_ids(h: &Harness<'_, App>) -> Vec<Uuid> {
    let page = std::sync::Arc::clone(h.state().shell().canvas().unwrap().page());
    page.tables
        .reps
        .iter()
        .filter(|r| r.terminal)
        .map(|r| r.id)
        .collect()
}

/// The window points of the page's terminals not in `before`, left to right.
fn new_terminals(h: &Harness<'_, App>, before: &[Uuid]) -> Vec<Pos2> {
    let page = std::sync::Arc::clone(h.state().shell().canvas().unwrap().page());
    let mut points: Vec<Pos2> = page
        .tables
        .reps
        .iter()
        .filter(|r| r.terminal && !before.contains(&r.id))
        .map(|r| {
            let a = page.snapper.page_to_sheet((r.body[0], r.body[1]));
            let b = page.snapper.page_to_sheet((r.body[2], r.body[3]));
            screen(h, ((a.0 + b.0) / 2, (a.1 + b.1) / 2))
        })
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x));
    points
}

/// Types `n` into the open form's How many, presses Enter (Place), lays the row with one click at the free grid point
/// (60, 30) and leaves the Place tool; the new terminals' window points.
fn place_row(h: &mut Harness<'static, App>, n: u32) -> Vec<Pos2> {
    let before = terminal_ids(h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|x| x.accesskit_node().label().as_deref() == Some("How many"))
        .expect("How many")
        .click();
    settle(h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.event(egui::Event::Text(n.to_string()));
    settle(h);
    h.key_press(Key::Enter);
    settle(h);
    assert_eq!(
        h.state().shell().tool().name(),
        "place",
        "the first terminal floats"
    );
    let at = grid(h, (60, 30));
    click_at(h, at, PointerButton::Primary, Modifiers::NONE);
    h.key_press(Key::Escape);
    settle(h);
    let points = new_terminals(h, &before);
    assert_eq!(points.len(), n as usize, "the row");
    points
}

/// Whether the terminal at `at`, clicked, is reported in its Properties' header as a device without a part.
fn without_part(h: &mut Harness<'static, App>, at: Pos2) -> bool {
    click_at(h, at, PointerButton::Primary, Modifiers::NONE);
    let sheet = h.state().shell().properties().sheet().expect("its properties");
    assert!(
        sheet.header.title.contains(':'),
        "a terminal: {}",
        sheet.header.title
    );
    sheet.header.findings.iter().any(|f| f.rule == "EW-CORE-041")
}

/// The menu entry whose label starts with `label` nearest the right click at `at` (Properties may name the same
/// command).
fn entry<'h>(h: &'h Harness<'_, App>, at: Pos2, label: &'h str) -> egui_kittest::Node<'h> {
    h.query_all_by_label_contains(label)
        .filter(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(label)))
        .min_by(|a, b| {
            let d =
                |n: &egui_kittest::Node<'_>| (n.rect().left() - at.x).abs() + (n.rect().top() - at.y).abs();
            d(a).total_cmp(&d(b))
        })
        .unwrap_or_else(|| panic!("{label} in the menu"))
}

/// Types the part number into the open part dialog's search and chooses the first offer by Enter.
fn choose_part(h: &mut Harness<'static, App>, number: &str) {
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::PartChoose(_))),
        "the part dialog"
    );
    h.event(egui::Event::Text(number.to_owned()));
    settle(h);
    h.key_press(Key::Enter);
    settle(h);
    assert!(h.state().shell().dialog().is_none(), "Enter chose");
}

/// The form proposes the starter part of its kind for a new strip (feed-through for Terminal), the PE kind proposes
/// the PE part, and a part chosen sets the kind it draws; three terminals placed with the proposal are no device
/// without a part.
#[test]
fn terminal_proposes_the_part_of_its_kind_and_its_row_has_a_part() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    open_form(&mut h);
    let (_, words) = form_choice(&h, "part");
    assert!(
        words.starts_with("EW-GEN-TRM-1"),
        "the feed-through part: {words}"
    );
    let (terminal, _) = form_choice(&h, "kind");
    choose(&mut h, "Kind", "Terminal, protective earth");
    let (pe, _) = form_choice(&h, "kind");
    assert_ne!(pe, terminal, "the PE kind chosen");
    let (_, words) = form_choice(&h, "part");
    assert!(words.starts_with("EW-GEN-TRM-2"), "the PE part: {words}");
    // The feed-through part sets its kind again.
    choose(&mut h, "Part", "EW-GEN-TRM-1");
    assert_eq!(form_choice(&h, "kind").0, terminal);
    for at in place_row(&mut h, 3) {
        assert!(!without_part(&mut h, at), "a terminal of the row has the part");
    }
    h.state_mut().shell_mut().shutdown();
}

/// Two terminals placed with No part are reported; both selected, the menu's Choose part… gives them the part and the
/// finding goes; Add terminal on one of them adds a terminal with the same part.
#[test]
fn a_terminals_menu_chooses_the_part_and_add_terminal_carries_it() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    open_form(&mut h);
    choose(&mut h, "Part", "No part");
    assert_eq!(form_choice(&h, "part").1, "No part");
    let points = place_row(&mut h, 2);
    assert!(without_part(&mut h, points[1]), "No part is reported");
    assert!(without_part(&mut h, points[0]), "No part is reported");
    click_at(&mut h, points[1], PointerButton::Primary, Modifiers::SHIFT);
    assert_eq!(h.state().shell().selection().items.len(), 2, "both selected");
    click_at(&mut h, points[0], PointerButton::Secondary, Modifiers::NONE);
    entry(&h, points[0], "Choose part…").click();
    settle(&mut h);
    choose_part(&mut h, "EW-GEN-TRM-1");
    for at in &points {
        assert!(!without_part(&mut h, *at), "both have the part");
    }
    // Add terminal on one: the new terminal has the part.
    let before = terminal_ids(&h);
    click_at(&mut h, points[0], PointerButton::Secondary, Modifiers::NONE);
    entry(&h, points[0], "Add terminal").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "place", "a terminal floats");
    let at = grid(&h, (60, 40));
    click_at(&mut h, at, PointerButton::Primary, Modifiers::NONE);
    h.key_press(Key::Escape);
    settle(&mut h);
    let added = new_terminals(&h, &before);
    assert_eq!(added.len(), 1);
    assert!(!without_part(&mut h, added[0]), "the added terminal has the part");
    h.state_mut().shell_mut().shutdown();
}

/// A strip's row in the Terminal strips navigator offers Choose part for its terminals without one, which gives them
/// all the part.
#[test]
fn a_strip_rows_menu_chooses_the_part_of_its_terminals_without_one() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    let tag = open_form(&mut h);
    choose(&mut h, "Part", "No part");
    let points = place_row(&mut h, 2);
    assert!(without_part(&mut h, points[0]), "No part is reported");
    dispatch(&mut h, "navigator.show", json!({ "kind": "strips" }));
    settle(&mut h);
    let name = h
        .state()
        .shell()
        .shown_rows(NavKind::Strips)
        .into_iter()
        .find(|r| r.role == "strip" && r.name.contains(&tag))
        .map(|r| r.name)
        .unwrap_or_else(|| panic!("the row of {tag}"));
    h.get_by_label(&name).click_secondary();
    settle(&mut h);
    h.get_by_label_contains("Choose part for 2 terminals").click();
    settle(&mut h);
    choose_part(&mut h, "EW-GEN-TRM-1");
    for at in &points {
        assert!(!without_part(&mut h, *at), "the strip's terminals have the part");
    }
    h.state_mut().shell_mut().shutdown();
}
