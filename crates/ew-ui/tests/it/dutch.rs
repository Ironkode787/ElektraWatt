// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 4 in the window (D12, NL-3, ruling 6): with the UI in Dutch (nl-NL) the first-run flow, the
//! core flows of LO-5 (new project, place and wire, a terminal strip, renumber, export PDF) and the Settings surfaces
//! (the language and keymap quick switches, View ▸ UI scale, the keyboard shortcuts) pass `assert_accessible` and
//! `assert_fits` after each step, at a UI scale of 100 % and of 200 %.

use std::path::PathBuf;
use std::time::Duration;

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::{App, Pick};
use ew_ui::ew_viewmodel::navigators::NavKind;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_ui::ew_viewmodel::{key, strings};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, harness_with, settle, wait_for_page};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    let out = h.state_mut().shell_mut().dispatch(action, &params);
    assert_eq!(out, Outcome::Done, "{action}: {out:?}");
}

/// One step of a flow in Dutch: the frame settles, every input is named and every node fits.
#[track_caller]
fn step(h: &mut Harness<'_, App>) {
    settle(h);
    support::assert_fits(h);
}

/// The window of the harness (1000 × 700 points) on a display of `scale` times its pixels.
fn display(h: &mut Harness<'_, App>, scale: f32) {
    h.set_size(egui::Vec2::new(1000.0, 700.0) * scale);
}

/// The UI in Dutch (View ▸ Language) at the UI scale `scale` (1.0 or 2.0, as `switch.rs` sets it), on a display
/// whose window is 1000 × 700 points at that scale.
#[track_caller]
fn dutch_at(h: &mut Harness<'_, App>, scale: f32) {
    dispatch(h, "view.language", json!({ "value": "nl" }));
    assert_eq!(strings::text(key!("menu-file")), "Bestand");
    dispatch(h, "view.ui_scale", json!({ "scale": scale }));
    display(h, scale);
    settle(h);
    step(h);
    assert!((h.ctx.pixels_per_point() - scale).abs() < 1e-6);
}

/// Golden A open on folio 101 in the window, in Dutch at `scale` (the setup of `place.rs`).
fn on_golden(tmp: &Tmp, scale: f32) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "101" }));
    settle(&mut h);
    wait_for_page(&mut h);
    dutch_at(&mut h, scale);
    h
}

/// The first-run flow of `part2.rs`, with Nederlands chosen on its first step: every later step is in Dutch.
fn first_run(scale: f32) {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    assert_eq!(
        shell.dispatch("view.ui_scale", &json!({ "scale": scale })),
        Outcome::Done
    );
    shell.start_first_run_if_needed();
    let mut h = harness_with(shell, Scripted::default(), None);
    display(&mut h, scale);
    settle(&mut h);
    step(&mut h);
    assert!((h.ctx.pixels_per_point() - scale).abs() < 1e-6);
    h.get_by_label(strings::text(key!("language-nl"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().settings().language, "nl");
    assert_eq!(strings::locale(), "nl");
    assert!(h.query_by_label("Welkom bij ElektraWatt").is_some());
    h.get_by_label(strings::text(key!("firstrun-next"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("origin-kicad"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("firstrun-skip"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("firstrun-next"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("theme-dark"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().settings().theme, "dark");
    h.get_by_label(strings::text(key!("firstrun-back"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("firstrun-next"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("firstrun-next"))).click();
    step(&mut h);
    h.get_by_label(strings::text(key!("firstrun-start-sample")))
        .click();
    step(&mut h);
    assert!(h.state().shell().first_run().is_none());
    assert_eq!(h.state().shell().project_name(), Some("J-E-2416.ew"));
    wait_for_page(&mut h);
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The new-project flow of `part2.rs`: the start page's New project, the folder dialog, the built-in page.
fn new_project(scale: f32) {
    let tmp = Tmp::new();
    let dir = tmp.0.join("first.ew");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(dir));
    let shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    let mut h = harness_with(shell, platform, None);
    settle(&mut h);
    dutch_at(&mut h, scale);
    h.get_by_label(strings::text(key!("start-new"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().project_name(), Some("first.ew"));
    wait_for_page(&mut h);
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The place and wire flows of `place.rs`: a symbol picked, its ghost, the placement; the Wire tool, its start, its
/// finish.
fn place_and_wire(scale: f32) {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, scale);
    dispatch(&mut h, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    step(&mut h);
    dispatch(&mut h, "canvas.pointer", json!({"x_mm": 120.0, "y_mm": 120.0}));
    step(&mut h);
    dispatch(&mut h, "canvas.press", json!({"x_mm": 120.0, "y_mm": 120.0}));
    step(&mut h);
    dispatch(&mut h, "tool.wire", json!({}));
    step(&mut h);
    dispatch(&mut h, "canvas.press", json!({"x_mm": 60.0, "y_mm": 150.0}));
    dispatch(&mut h, "canvas.pointer", json!({"x_mm": 100.0, "y_mm": 150.0}));
    step(&mut h);
    dispatch(&mut h, "canvas.press", json!({"x_mm": 100.0, "y_mm": 150.0}));
    dispatch(&mut h, "tool.finish", json!({}));
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The terminal-strip flow of `table_mode.rs`: the strip editor on -X1031 and a cell of its grid.
fn terminal_strip(scale: f32) {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, scale);
    dispatch(&mut h, "strip.edit", json!({"strip": "-X1031"}));
    step(&mut h);
    let row = {
        let n = h.state().shell().nav(NavKind::Strips).expect("the navigator");
        let t = n.table().expect("its table");
        let node = n.visible().get(1).expect("a terminal row").node;
        t.nodes[node as usize].key.clone()
    };
    dispatch(&mut h, "table.cell", json!({"kind": "strips", "row": row}));
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The renumber flow of `preview_history.rs`: Renumber devices…, its dialog, the Preview tab, Apply.
fn renumber(scale: f32) {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, scale);
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": {"default": "page-based",
            "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}", "rules": [], "child_separator": "-"}]}}}),
    );
    dispatch(&mut h, "devices.renumber", json!({}));
    step(&mut h);
    support::off_chrome(&h, strings::text(key!("dialog-renumber")))
        .expect("the dialog's Hernummeren")
        .click();
    step(&mut h);
    h.get_by_label(strings::text(key!("preview-apply"))).click();
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The export flow of `jobs.rs`: Export PDF through the save dialog, the job, its toast and the Jobs pane.
fn export_pdf(scale: f32) {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let pdf: PathBuf = tmp.0.join("out.pdf");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(pdf.clone()));
    let mut h = harness(&tmp, platform, Some(dir));
    settle(&mut h);
    dutch_at(&mut h, scale);
    dispatch(&mut h, "file.export_pdf", Value::Null);
    for _ in 0..3000 {
        h.run_ok();
        if h.state().shell().jobs().running() == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(h.state().shell().jobs().running(), 0, "the export ended");
    step(&mut h);
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
    dispatch(&mut h, "jobs.show", json!({}));
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The Settings surfaces: the start page's language and keymap quick switches, View ▸ UI scale and the keyboard
/// shortcuts.
fn settings(scale: f32) {
    let tmp = Tmp::new();
    let shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    dutch_at(&mut h, scale);
    h.get_by_label(strings::text(key!("language-en"))).click();
    step(&mut h);
    assert_eq!(strings::locale(), "en");
    h.get_by_label(strings::text(key!("language-nl"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().settings().language, "nl");
    h.get_by_label(strings::text(key!("keymap-kicad"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().settings().keymap, "kicad");
    h.get_by_label(strings::text(key!("keymap-ew"))).click();
    step(&mut h);
    support::open_menu(
        &mut h,
        &[
            strings::text(key!("menu-view")),
            strings::text(key!("submenu-ui-scale")),
        ],
    );
    support::assert_fits(&h);
    let label = if scale == 1.0 {
        strings::text(key!("scale-100"))
    } else {
        strings::text(key!("scale-200"))
    };
    support::menu_node(&h, label).click();
    step(&mut h);
    assert_eq!(h.state().shell().settings().ui_scale, scale);
    dispatch(&mut h, "help.shortcuts", json!({}));
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn nl3_the_first_run_in_dutch_at_100() {
    first_run(1.0);
}

#[test]
fn nl3_the_first_run_in_dutch_at_200() {
    first_run(2.0);
}

#[test]
fn nl3_new_project_in_dutch_at_100() {
    new_project(1.0);
}

#[test]
fn nl3_new_project_in_dutch_at_200() {
    new_project(2.0);
}

#[test]
fn nl3_place_and_wire_in_dutch_at_100() {
    place_and_wire(1.0);
}

#[test]
fn nl3_place_and_wire_in_dutch_at_200() {
    place_and_wire(2.0);
}

#[test]
fn nl3_a_terminal_strip_in_dutch_at_100() {
    terminal_strip(1.0);
}

#[test]
fn nl3_a_terminal_strip_in_dutch_at_200() {
    terminal_strip(2.0);
}

#[test]
fn nl3_renumber_in_dutch_at_100() {
    renumber(1.0);
}

#[test]
fn nl3_renumber_in_dutch_at_200() {
    renumber(2.0);
}

#[test]
fn nl3_export_pdf_in_dutch_at_100() {
    export_pdf(1.0);
}

#[test]
fn nl3_export_pdf_in_dutch_at_200() {
    export_pdf(2.0);
}

#[test]
fn nl3_the_settings_surfaces_in_dutch_at_100() {
    settings(1.0);
}

#[test]
fn nl3_the_settings_surfaces_in_dutch_at_200() {
    settings(2.0);
}

/// Every AccessKit label of the window's last frame.
fn labels(h: &Harness<'_, App>) -> Vec<String> {
    h.root()
        .children_recursive()
        .filter_map(|n| n.accesskit_node().label())
        .collect()
}

/// ID-1278 ruling 5 (W49.1 I): after View ▸ Language back to English, the very next frame's Pages navigator speaks
/// English (its rows were kept from the Dutch frame until the next change): in tree mode its section row and the
/// `generated` badge (the grid's badge reads "QET 10 px" in both languages, as the status bar), in table mode its
/// cells' row positions.
#[test]
fn the_pages_navigator_follows_the_language_in_its_first_frame() {
    let modes: [(bool, &[&str], &[&str]); 2] = [
        (false, &["Sectie 1", "gegenereerd"], &["Section 1", "generated"]),
        (true, &[", rij 1 van 36"], &[", row 1 of 36"]),
    ];
    for (table, dutch_words, english_words) in modes {
        let tmp = Tmp::new();
        let mut h = on_golden(&tmp, 1.0);
        // Tall enough that every page row is painted, the generated report pages among them.
        h.set_size(egui::Vec2::new(1000.0, 3000.0));
        if table {
            dispatch(&mut h, "navigator.table", json!({ "kind": "pages" }));
        }
        settle(&mut h);
        let shows = |h: &Harness<'_, App>, word: &str| labels(h).iter().any(|l| l.contains(word));
        for word in dutch_words {
            assert!(shows(&h, word), "table {table}: {word} in {:?}", labels(&h));
        }
        dispatch(&mut h, "view.language", json!({ "value": "en" }));
        h.step();
        for word in english_words {
            assert!(shows(&h, word), "table {table}: {word} in {:?}", labels(&h));
        }
        for word in dutch_words {
            assert!(!shows(&h, word), "table {table}: {word} is gone");
        }
        h.state_mut().shell_mut().shutdown();
    }
}
