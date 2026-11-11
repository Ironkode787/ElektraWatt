// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 3 in the window (D10, D11, NFR-I18N-02). LO-5: in the pseudo-locale en-XA (every text accented,
//! 35 % longer and bracketed) the core flows of the kittest suite, with the setups of their tests (new project, place,
//! wire, a terminal strip, renumber, export PDF), pass `assert_accessible` and `assert_fits` after each step. LO-2: View ▸
//! Language ▸ Nederlands switches the menus, the command bar and an open card without a restart, and back.

use std::path::PathBuf;
use std::time::Duration;

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::{App, Pick};
use ew_ui::ew_viewmodel::navigators::NavKind;
use ew_ui::ew_viewmodel::shell::{Mode, ShellModel};
use ew_ui::ew_viewmodel::{key, strings};
use serde_json::{Value, json};
use support::{Scripted, Tmp, dispatch, harness, harness_with, settle, wait_for_page};

/// Whether a tab (the command bar's) is labelled `label`.
fn bar_tab(h: &Harness<'_, App>, label: &str) -> bool {
    h.query_all_by_label(label)
        .any(|n| n.accesskit_node().role() == egui::accesskit::Role::Tab)
}

/// One step of a flow in en-XA: the frame settles, every input is named and every node fits.
#[track_caller]
fn step(h: &mut Harness<'_, App>) {
    settle(h);
    support::assert_fits(h);
}

/// Golden A open on folio 101 in the window, then the pseudo-locale (the setup of `place.rs`).
fn on_golden(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "101" }));
    settle(&mut h);
    wait_for_page(&mut h);
    assert!(strings::set_locale(strings::PSEUDO));
    step(&mut h);
    h
}

/// The new-project flow of `shell_frame.rs`: the start page's New project, the folder dialog, the built-in page.
#[test]
fn new_project_in_the_pseudo_locale() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("first.ew");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(dir));
    let shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    let mut h = harness_with(shell, platform, None);
    assert!(strings::set_locale(strings::PSEUDO));
    step(&mut h);
    h.get_by_label(strings::text(key!("start-new"))).click();
    step(&mut h);
    assert_eq!(h.state().shell().project_name(), Some("first.ew"));
    wait_for_page(&mut h);
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The fit check finds what does not fit: the start page in a window narrower than its buttons.
#[test]
fn the_fit_check_finds_a_node_that_leaves_the_window() {
    let tmp = Tmp::new();
    let shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    support::assert_fits(&h);
    h.set_size(egui::Vec2::new(120.0, 700.0));
    settle(&mut h);
    let problems = support::fit_problems(&h);
    assert!(
        problems.iter().any(|p| p.ends_with(" leaves the window")),
        "{problems:#?}"
    );
}

/// A fixture: the buttons "One" and "Two", 45 × 25 each, side by side `gap` apart (negative: overlapping) in a parent
/// bounded at 20, 20 (100 × 40), in a 400 × 300 window.
fn side_by_side(gap: f32) -> Harness<'static> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .with_context_setup(ew_ui::fonts::install)
        .build_ui(move |ui| {
            let frame = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(100.0, 40.0));
            let builder = egui::UiBuilder::new()
                .max_rect(frame)
                .layout(egui::Layout::left_to_right(egui::Align::Min));
            ui.scope_builder(builder, |inner| {
                let id = inner.unique_id();
                inner.ctx().accesskit_node_builder(id, |n| {
                    n.set_bounds(egui::accesskit::Rect::new(20.0, 20.0, 120.0, 60.0));
                });
                inner.spacing_mut().item_spacing.x = gap;
                for name in ["One", "Two"] {
                    inner.add(egui::Button::new(name).min_size(egui::vec2(45.0, 25.0)));
                }
            });
        });
    h.run();
    h
}

/// The fit check finds a node cut across by its bounded parent, and only that.
#[test]
fn the_fit_check_finds_a_node_that_leaves_its_parent() {
    assert_eq!(
        support::fit_problems(&side_by_side(30.0)),
        [
            "Button \"Two\" at Rect { x0: 95.0, y0: 20.0, x1: 140.0, y1: 45.0 } leaves its parent at \
             Rect { x0: 20.0, y0: 20.0, x1: 120.0, y1: 60.0 }"
        ]
    );
}

/// The fit check finds two interactive siblings that overlap, and only that.
#[test]
fn the_fit_check_finds_two_siblings_that_overlap() {
    assert_eq!(
        support::fit_problems(&side_by_side(-10.0)),
        [
            "Button \"Two\" at Rect { x0: 55.0, y0: 20.0, x1: 100.0, y1: 45.0 } overlaps \
             Button \"One\" at Rect { x0: 20.0, y0: 20.0, x1: 65.0, y1: 45.0 }"
        ]
    );
}

/// The place and wire flows of `place.rs`: a symbol picked, its ghost, the placement; the Wire tool, its start, its
/// finish.
#[test]
fn place_and_wire_in_the_pseudo_locale() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
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
#[test]
fn a_terminal_strip_in_the_pseudo_locale() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
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
#[test]
fn renumber_in_the_pseudo_locale() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    support::page_based_numbering(&mut h);
    dispatch(&mut h, "devices.renumber", json!({}));
    step(&mut h);
    support::off_chrome(&h, strings::text(key!("dialog-renumber")))
        .expect("the dialog's Renumber")
        .click();
    step(&mut h);
    h.get_by_label(strings::text(key!("preview-apply"))).click();
    step(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The export flow of `jobs.rs`: Export PDF through the save dialog, the job, its toast and the Jobs pane.
#[test]
fn export_pdf_in_the_pseudo_locale() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let pdf: PathBuf = tmp.0.join("out.pdf");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(pdf.clone()));
    let mut h = harness(&tmp, platform, Some(dir));
    settle(&mut h);
    assert!(strings::set_locale(strings::PSEUDO));
    step(&mut h);
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

/// LO-2: with the shipped nl-NL, View ▸ Language ▸ Nederlands renames the View menu and the
/// command bar's tabs ("Beeld", "Pagina") and the open preview card's Apply ("Toepassen") without a restart, and
/// View ▸ Taal ▸ English brings them back.
#[test]
fn view_language_nederlands_and_back_in_the_window() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    // An open card: the renumber preview of `preview_history.rs`.
    support::page_based_numbering(&mut h);
    dispatch(&mut h, "devices.renumber", json!({}));
    settle(&mut h);
    support::off_chrome(&h, "Renumber")
        .expect("the dialog's Renumber")
        .click();
    settle(&mut h);
    assert!(h.query_by_label("Apply").is_some(), "the open card in English");
    support::open_menu(&mut h, &["View", "Language"]);
    support::menu_node(&h, "Nederlands").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().language, "nl");
    assert!(h.query_by_label("Toepassen").is_some(), "the open card in Dutch");
    assert!(h.query_by_label("Apply").is_none());
    assert!(bar_tab(&h, "Beeld"), "the command bar's View tab");
    assert!(bar_tab(&h, "Pagina"), "the command bar's Page tab");
    assert!(!bar_tab(&h, "Page"));
    support::open_menu(&mut h, &["Beeld", "Taal"]);
    support::menu_node(&h, "English").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().language, "en");
    assert!(h.query_by_label("Apply").is_some());
    assert!(bar_tab(&h, "View"));
    assert!(h.query_by_label("Beeld").is_none());
    h.state_mut().shell_mut().shutdown();
}
