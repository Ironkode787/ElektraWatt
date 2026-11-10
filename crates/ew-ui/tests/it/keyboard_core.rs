// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 5b, acceptance check AX-5 (NFR-A11Y-01): the five core tasks on a copy of Golden A with key events
//! only: place a symbol (I, the picker by typing and arrows, Enter, Enter at the grid cursor), draw a
//! wire (W, `]`, Enter, `]`, `]`, Enter), insert a terminal from the Terminal strips navigator (Ctrl+2; F6 and
//! Shift+F6; Insert), renumber the devices through the palette and Ctrl+Enter (Ctrl+K, "renum", Enter; Tab and Space
//! clear "Only non-conforming tags"; Ctrl+Enter submits; F6 to Properties and Ctrl+Enter apply the card) and export the PDF
//! (Ctrl+Shift+E, the save dialog answered with a path in the test's folder). Each step's effect is read from the
//! model; the accessible-name walker runs after every key.

use crate::support;

use std::collections::BTreeSet;
use std::time::Duration;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_ui::app::{App, Pick};
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::properties::CardOrigin;
use ew_ui::ew_viewmodel::shell::{Dialog, Focus, Prompt};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn press(h: &mut Harness<'_, App>, key: Key) {
    h.key_press(key);
    settle(h);
}

fn chord(h: &mut Harness<'_, App>, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    settle(h);
}

/// Types `text` as keyboard input into whatever has the focus.
fn type_keys(h: &mut Harness<'_, App>, text: &str) {
    h.event(egui::Event::Text(text.to_owned()));
    settle(h);
}

/// `]` as the keyboard sends it: the physical key right of P.
fn bracket(h: &mut Harness<'_, App>) {
    h.event(egui::Event::Key {
        key: Key::CloseBracket,
        physical_key: Some(Key::CloseBracket),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    settle(h);
}

/// The representations on the shown page, by symbol key.
fn reps_by_symbol(h: &Harness<'_, App>) -> Vec<(String, String)> {
    h.state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| (r.id.to_string(), r.symbol_key.clone()))
        .collect()
}

/// The connections drawn on the shown page.
fn connections(h: &Harness<'_, App>) -> BTreeSet<String> {
    h.state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .map(|c| c.id.to_string())
        .collect()
}

/// The terminal rows of the Terminal strips navigator under the strip labelled `strip`.
fn terminals_of(h: &Harness<'_, App>, strip: &str) -> usize {
    let nav = h.state().shell().nav(NavKind::Strips).expect("the navigator");
    let table = nav.table().expect("its rows");
    let Some(s) = table
        .nodes
        .iter()
        .position(|n| n.role == NavRole::Strip && n.label == strip)
    else {
        return 0;
    };
    table
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Terminal && n.parent == Some(s as u32))
        .count()
}

#[test]
fn ax5_the_five_core_tasks_with_key_events_only() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let pdf = tmp.0.join("out.pdf");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(pdf.clone()));
    let mut h = harness(&tmp, platform.clone(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);

    // Folio 103: Ctrl+G, the folio, Enter.
    chord(&mut h, Modifiers::COMMAND, Key::G);
    type_keys(&mut h, "103");
    press(&mut h, Key::Enter);
    wait_for_page(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().current_folio(), Some("103"));
    assert!(
        matches!(h.state().shell().focus(), Some(Focus::Canvas(_))),
        "{:?}",
        h.state().shell().focus()
    );

    // 1. A symbol: I opens the picker, typing filters it, Down moves its row, Enter chooses; the arrows move the grid
    // cursor and Enter places the symbol there.
    let before = reps_by_symbol(&h);
    press(&mut h, Key::I);
    assert!(h.state().shell().picker().is_some(), "I opens the picker");
    type_keys(&mut h, "contact");
    let rows = h.state().shell().picker().unwrap().rows.len();
    assert!(rows > 1, "{rows} rows");
    press(&mut h, Key::ArrowDown);
    assert_eq!(h.state().shell().picker().unwrap().row, 1);
    press(&mut h, Key::Enter);
    assert!(h.state().shell().picker().is_none());
    assert_eq!(h.state().shell().tool().name(), "place");
    for _ in 0..12 {
        press(&mut h, Key::ArrowRight);
    }
    for _ in 0..8 {
        press(&mut h, Key::ArrowDown);
    }
    press(&mut h, Key::Enter);
    let after = reps_by_symbol(&h);
    let new: Vec<&(String, String)> = after.iter().filter(|r| !before.contains(r)).collect();
    assert_eq!(new.len(), 1, "one representation placed: {new:?}");
    let symbol = &new[0].1;
    let count = |reps: &[(String, String)]| reps.iter().filter(|(_, s)| s == symbol).count();
    assert_eq!(count(&after), count(&before) + 1, "{symbol}");
    press(&mut h, Key::Escape);
    assert_eq!(h.state().shell().tool().name(), "select");

    // 2. A wire: W, `]` to a connection point, Enter, `]` twice, Enter.
    let before = connections(&h);
    press(&mut h, Key::W);
    assert_eq!(h.state().shell().tool().name(), "wire");
    bracket(&mut h);
    press(&mut h, Key::Enter);
    bracket(&mut h);
    bracket(&mut h);
    press(&mut h, Key::Enter);
    let after = connections(&h);
    assert_eq!(after.difference(&before).count(), 1, "one wire drawn");
    press(&mut h, Key::Escape);

    // 3. A terminal: Ctrl+2 shows the Terminal strips navigator (the rail's second entry) with the focus; F6 moves
    // the focus on and Shift+F6 back; Insert inserts a terminal before the cursor's.
    chord(&mut h, Modifiers::COMMAND, Key::Num2);
    let strips = Some(Focus::Navigator(NavKind::Strips));
    assert_eq!(h.state().shell().focus(), strips);
    press(&mut h, Key::F6);
    assert_ne!(h.state().shell().focus(), strips);
    chord(&mut h, Modifiers::SHIFT, Key::F6);
    assert_eq!(h.state().shell().focus(), strips);
    let strip = {
        let nav = h.state().shell().nav(NavKind::Strips).unwrap();
        let table = nav.table().unwrap();
        table
            .nodes
            .iter()
            .find(|n| n.role == NavRole::Strip)
            .map(|n| n.label.clone())
            .expect("a strip")
    };
    let before = terminals_of(&h, &strip);
    press(&mut h, Key::Insert);
    assert_eq!(
        terminals_of(&h, &strip),
        before + 1,
        "a terminal inserted into {strip}"
    );

    // 4. Renumber devices…: Ctrl+K, "renum", Enter opens its form; Ctrl+Enter submits it and opens
    // the renumber card, which F6 to Properties and Ctrl+Enter apply.
    chord(&mut h, Modifiers::COMMAND, Key::K);
    assert!(h.state().shell().palette().is_some());
    type_keys(&mut h, "renum");
    press(&mut h, Key::Enter);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "devices.renumber"),
        "the renumber form"
    );
    // Tab to "Only non-conforming tags", Space clears it: every device is renumbered.
    let mut tabs = 0;
    while !h.get_all_by_role(egui::Role::CheckBox).any(|n| {
        n.accesskit_node().label().as_deref() == Some("Only non-conforming tags")
            && n.accesskit_node().is_focused()
    }) {
        tabs += 1;
        assert!(tabs <= 8, "Tab reaches Only non-conforming tags");
        press(&mut h, Key::Tab);
    }
    press(&mut h, Key::Space);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::Form(f))
            if f.fields.iter().any(|f| f.name == "only_nonconforming" && f.value == "false")),
        "Space cleared it"
    );
    chord(&mut h, Modifiers::COMMAND, Key::Enter);
    assert!(
        h.state().shell().dialog().is_none(),
        "Ctrl+Enter submitted the form"
    );
    let card = h.state().shell().properties().card().expect("the renumber card");
    assert!(matches!(card.origin, CardOrigin::Bulk));
    assert_eq!(card.preview.name, "numbering.renumber");
    assert!(card.changes > 0, "the card renumbers devices");
    // F6 to Properties, whose Ctrl+Enter applies the card: one renumber.
    let steps = h.state().shell().recorded_commands().len();
    for _ in 0..8 {
        if h.state().shell().focus() == Some(Focus::Properties) {
            break;
        }
        press(&mut h, Key::F6);
    }
    chord(&mut h, Modifiers::COMMAND, Key::Enter);
    assert!(h.state().shell().properties().card().is_none(), "applied");
    assert_eq!(h.state().shell().recorded_commands().len(), steps + 1);
    assert_eq!(
        h.state()
            .shell()
            .recorded_commands()
            .last()
            .map(|c| c.cmd.as_str()),
        Some("numbering.renumber")
    );

    // 5. Export PDF: Ctrl+Shift+E, the save dialog's answer, the export job to its end.
    chord(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::E);
    for _ in 0..3000 {
        h.run_ok();
        if h.state().shell().jobs().running() == 0 && pdf.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    settle(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::SavePdf]);
    let bytes = std::fs::read(&pdf).expect("the PDF");
    assert!(bytes.starts_with(b"%PDF-"), "a PDF");
    h.state_mut().shell_mut().shutdown();
}
