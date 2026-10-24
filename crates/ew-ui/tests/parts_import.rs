// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File, Import, Parts table... in the window without rendering (FT-410): the entry is disabled without a project;
//! with a new project it raises the file prompt (`Prompt::OpenPartsTable`), the answer opens the import card
//! ("parts.csv: 29 created, 0 skipped, 0 errors"), Ctrl+Enter applies it from anywhere, and the project has the 29
//! part copies; the accessible-name walker after every step.

mod support;

use std::path::{Path, PathBuf};

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use egui::{Key, Modifiers, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::{App, Pick, Platform, setup};
use ew_ui::ew_viewmodel::ew_api::Session;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, Prompt, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, harness_with, settle, wait_for_page};

fn golden_parts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv")
}

#[test]
fn without_a_project_the_entry_is_disabled() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    h.get_by_label("File").click();
    settle(&mut h);
    h.get_by_label("Import").click();
    settle(&mut h);
    assert!(
        h.get_by_label("Parts table...").accesskit_node().is_disabled(),
        "no project: disabled"
    );
}

#[test]
fn the_entry_prompts_and_ctrl_enter_applies_the_card() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("proj.ew");
    let platform = Scripted::default();
    platform
        .answers
        .borrow_mut()
        .push_back(Pick::Path(golden_parts()));
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let params: Value = json!({"dir": dir.to_string_lossy(), "template": "builtin"});
    assert_eq!(shell.dispatch("file.new", &params), Outcome::Done);
    let mut h = harness_with(shell, platform.clone(), None);
    settle(&mut h);
    wait_for_page(&mut h);

    h.get_by_label("File").click();
    settle(&mut h);
    h.get_by_label("Import").click();
    settle(&mut h);
    assert!(
        !h.get_by_label("Parts table...").accesskit_node().is_disabled(),
        "a writable project: enabled"
    );
    h.get_by_label("Parts table...").click();
    settle(&mut h);
    settle(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::OpenPartsTable]);
    // The card: a window with the counts and the two buttons.
    assert!(
        h.query_by_label_contains("parts.csv: 29 created").is_some(),
        "the card's line"
    );
    assert!(h.query_by_label("Import parts").is_some(), "the window's title");
    assert!(h.query_by_label("Apply").is_some());
    assert!(h.query_by_label("Cancel").is_some());
    assert!(h.state().shell().import_card_open());
    assert!(!h.state().shell().can_undo(), "the card waits for Apply");

    // Ctrl+Enter applies it.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert!(
        h.state().shell().properties().card().is_none(),
        "the card is gone"
    );
    assert!(h.query_by_label_contains("parts.csv: 29 created").is_none());
    assert!(h.state().shell().can_undo());
    assert_eq!(
        h.state().shell().banner(),
        Some("Imported 29 parts from parts.csv (0 skipped).")
    );
    // Saved, closed and read back: the 29 part copies.
    let shell = h.state_mut().shell_mut();
    assert_eq!(shell.dispatch("file.save", &Value::Null), Outcome::Done);
    assert_eq!(shell.dispatch("file.close", &Value::Null), Outcome::Done);
    let (reopened, _) = Session::open_read_only(&dir, "0.0.0-test").unwrap();
    assert_eq!(reopened.query().parts().iter().count(), 29);
}

#[test]
fn cancel_drops_the_card_without_a_change() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("proj.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let params: Value = json!({"dir": dir.to_string_lossy(), "template": "builtin"});
    assert_eq!(shell.dispatch("file.new", &params), Outcome::Done);
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    wait_for_page(&mut h);
    let path = golden_parts();
    let out = h
        .state_mut()
        .shell_mut()
        .dispatch("parts.import", &json!({"path": path.to_string_lossy()}));
    assert_eq!(out, Outcome::Done);
    settle(&mut h);
    h.get_by_label("Cancel").click();
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    assert!(!h.state().shell().can_undo());
    h.state_mut().shell_mut().shutdown();
}

/// W3: the next frame after the file dialog returns, with no pointer input, shows the menu closed and the import card;
/// A platform that records the egui pass in which it answered the dialog, so the test can inspect that exact frame.
struct DialogPlatform {
    path: PathBuf,
    ctx: Rc<RefCell<Option<egui::Context>>>,
    pass: Rc<Cell<u64>>,
}

impl Platform for DialogPlatform {
    fn pick(&mut self, _: Prompt) -> Pick {
        if let Some(ctx) = self.ctx.borrow().as_ref() {
            self.pass.set(ctx.cumulative_pass_nr());
        }
        Pick::Path(self.path.clone())
    }
}

/// W3: the frame that answers the file dialog, with no pointer input, has the menu closed and the import card in the
/// shell; the following frame paints its text, and Apply's next frame shows the banner.
#[test]
fn the_card_and_the_closed_menu_arrive_in_the_frame_after_the_pick() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("proj.ew");
    let csv = tmp.0.join("parts.csv");
    std::fs::write(
        &csv,
        "key,manufacturer,part_number,kind,type_designation,description,unit,test_data,tech_data\nA1,Acme,A1,,,Relay A,piece,,\nA2,Acme,A2,,,Relay B,piece,,\n",
    )
    .unwrap();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let params: Value = json!({"dir": dir.to_string_lossy(), "template": "builtin"});
    assert_eq!(shell.dispatch("file.new", &params), Outcome::Done);
    let ctx_slot = Rc::new(RefCell::new(None));
    let pick_pass = Rc::new(Cell::new(u64::MAX));
    let platform = DialogPlatform {
        path: csv,
        ctx: Rc::clone(&ctx_slot),
        pass: Rc::clone(&pick_pass),
    };
    let app = App::new(shell, Box::new(platform), None);
    let mut h = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .with_context_setup(setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    *ctx_slot.borrow_mut() = Some(h.ctx.clone());
    settle(&mut h);

    h.get_by_label("File").click();
    settle(&mut h);
    h.get_by_label("Import").click();
    settle(&mut h);
    h.get_by_label("Parts table...").click();
    // Step frames until the one in which the dialog was answered; it must already show the menu closed and the card in
    // the shell (the card's text is laid out in the following frame).
    let mut answered = false;
    for _ in 0..6 {
        let before = h.ctx.cumulative_pass_nr();
        h.step();
        let after = h.ctx.cumulative_pass_nr();
        let at = pick_pass.get();
        if at != u64::MAX && before <= at && at <= after {
            assert!(
                h.query_by_label("Parts table...").is_none(),
                "the File menu closes in the frame that answers the pick"
            );
            assert!(
                h.state().shell().import_card_open(),
                "the card is in the shell in the frame that answers the pick"
            );
            answered = true;
            break;
        }
    }
    assert!(answered, "the pick was answered");
    h.step();
    assert!(
        h.query_by_label_contains("parts.csv: 2 created").is_some(),
        "the card's text is painted"
    );
    h.get_by_label("Apply").click();
    h.step();
    assert_eq!(
        h.state().shell().banner(),
        Some("Imported 2 parts from parts.csv (0 skipped).")
    );
    h.step();
    assert!(
        h.query_by_label_contains("Imported 2 parts from parts.csv (0 skipped).")
            .is_some(),
        "the banner is painted"
    );
    assert!(
        h.query_by_label_contains("parts.csv: 2 created").is_none(),
        "the card is gone"
    );
    h.state_mut().shell_mut().shutdown();
}
