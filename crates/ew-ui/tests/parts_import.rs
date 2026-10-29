// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File, Import, Parts table… in the window without rendering (FT-410): the entry is disabled without a project;
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
use ew_ui::app::{App, Pick, PickReply, Platform, setup};
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
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    assert!(
        h.get_by_label("Parts table…").accesskit_node().is_disabled(),
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

    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    assert!(
        !h.get_by_label("Parts table…").accesskit_node().is_disabled(),
        "a writable project: enabled"
    );
    h.get_by_label("Parts table…").click();
    settle(&mut h);
    settle(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::OpenPartsTable]);
    // The card: a window with the counts and the two buttons.
    assert!(
        h.query_by_label_contains("parts.csv: 29 created").is_some(),
        "the card's line"
    );
    assert!(
        h.query_all_by_label_contains("Preview: Import parts")
            .next()
            .is_some(),
        "the card's title (in the strip while the Preview tab shows it)"
    );
    // WP-S2.09 part 3: the Preview tab shows the card too; its chip and the tab each have the button.
    assert!(h.get_all_by_label("Apply").next().is_some());
    assert!(h.get_all_by_label("Cancel").next().is_some());
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
    // WP-S2.09 part 3: the Preview tab shows the card too; its chip and the tab each have the button.
    h.get_all_by_label("Cancel").next().unwrap().click();
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

    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    h.get_by_label("Parts table…").click();
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
                h.query_by_label("Parts table…").is_none(),
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
    // WP-S2.09 part 3: the Preview tab shows the card too; its chip and the tab each have the button.
    h.get_all_by_label("Apply").next().unwrap().click();
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

/// FT-467: a platform whose dialog stays open off the window's thread until the test answers it through the sender.
#[derive(Clone, Default)]
struct ThreadedPlatform {
    answer: Rc<RefCell<Option<std::sync::mpsc::Sender<Pick>>>>,
    started: Rc<Cell<usize>>,
}

impl Platform for ThreadedPlatform {
    fn pick(&mut self, _: Prompt) -> Pick {
        panic!("a threaded platform is asked through start_pick");
    }

    fn start_pick(&mut self, _: Prompt, _: &egui::Context) -> PickReply {
        let (tx, rx) = std::sync::mpsc::channel();
        *self.answer.borrow_mut() = Some(tx);
        self.started.set(self.started.get() + 1);
        PickReply::Later(rx)
    }
}

/// A harness on a new project whose File, Import, Parts table… has been clicked once with `platform`.
fn harness_with_open_dialog<'a>(tmp: &Tmp, platform: ThreadedPlatform) -> Harness<'a, App> {
    let dir = tmp.0.join("proj.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let params: Value = json!({"dir": dir.to_string_lossy(), "template": "builtin"});
    assert_eq!(shell.dispatch("file.new", &params), Outcome::Done);
    let app = App::new(shell, Box::new(platform), None);
    let mut h = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .with_context_setup(setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    settle(&mut h);
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    h.get_by_label("Parts table…").click();
    h.step();
    h.step();
    h
}

/// FT-467: while the dialog is open on its own thread the window keeps painting, nothing is dispatched and no second
/// dialog starts; the frame after the answer arrives has the card in the shell, and the next one paints it.
#[test]
fn an_answer_from_the_dialog_thread_opens_the_card_in_the_next_frame() {
    let tmp = Tmp::new();
    let csv = tmp.0.join("parts.csv");
    std::fs::write(
        &csv,
        "key,manufacturer,part_number,kind,type_designation,description,unit,test_data,tech_data\nA1,Acme,A1,,,Relay A,piece,,\nA2,Acme,A2,,,Relay B,piece,,\n",
    )
    .unwrap();
    let platform = ThreadedPlatform::default();
    let mut h = harness_with_open_dialog(&tmp, platform.clone());
    assert_eq!(platform.started.get(), 1, "the dialog started once");
    let pass = h.ctx.cumulative_pass_nr();
    for _ in 0..3 {
        h.step();
    }
    assert!(
        h.ctx.cumulative_pass_nr() > pass,
        "the window keeps painting while the dialog is open"
    );
    assert!(!h.state().shell().import_card_open(), "no card before the answer");
    assert_eq!(platform.started.get(), 1, "no second dialog while one is open");
    assert!(
        h.query_by_label("Parts table…").is_none(),
        "the File menu is closed while the dialog is open"
    );

    let tx = platform.answer.borrow_mut().take().unwrap();
    tx.send(Pick::Path(csv)).unwrap();
    h.step();
    assert!(
        h.state().shell().import_card_open(),
        "the card is in the shell in the frame after the answer"
    );
    h.step();
    assert!(
        h.query_by_label_contains("parts.csv: 2 created").is_some(),
        "the card's text is painted"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-467: a dialog thread that ends without an answer (it panicked) counts as a cancel: no card, no change, and the
/// next prompt starts a new dialog.
#[test]
fn a_dialog_thread_without_an_answer_counts_as_cancelled() {
    let tmp = Tmp::new();
    let platform = ThreadedPlatform::default();
    let mut h = harness_with_open_dialog(&tmp, platform.clone());
    drop(platform.answer.borrow_mut().take());
    h.step();
    h.step();
    assert!(!h.state().shell().import_card_open());
    assert!(!h.state().shell().can_undo());
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    h.get_by_label("Parts table…").click();
    h.step();
    h.step();
    assert_eq!(platform.started.get(), 2, "a new prompt starts a new dialog");
    h.state_mut().shell_mut().shutdown();
}

/// The window position (points) of the sheet point `p` on the focused canvas.
fn screen(h: &Harness<'_, App>, p: (i64, i64)) -> egui::Pos2 {
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    egui::Pos2::new(
        bounds.x0 as f32 + ((p.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((p.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    )
}

/// FT-467 review: while a file dialog is open off the window's thread no input reaches the shell (a dialog that is not
/// modal, as on Linux, would otherwise let edits be made that the answer's `discard` throws away unasked): a key, a
/// menu action and a canvas click change nothing and dispatch nothing, the window keeps painting, and the answer then
/// proceeds with the open.
#[test]
fn no_input_reaches_the_shell_while_a_file_dialog_is_open() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let other = tmp.golden("h.ew");
    let platform = ThreadedPlatform::default();
    let shell = ShellModel::new(Mode::Gui, tmp.config());
    let app = App::new(shell, Box::new(platform.clone()), Some(dir));
    let mut h = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .with_context_setup(setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    settle(&mut h);
    wait_for_page(&mut h);
    let run = json!({"cmd": "page.create", "params": {"number": 999}});
    let goto = json!({"folio": "101"});
    let pick = json!({"symbol": "ew.iec.mcb-1p@qet10"});
    let shell = h.state_mut().shell_mut();
    assert_eq!(shell.dispatch("command.run", &run), Outcome::Done);
    assert_eq!(shell.dispatch("page.goto", &goto), Outcome::Done);
    settle(&mut h);
    wait_for_page(&mut h);
    assert_eq!(
        h.state_mut().shell_mut().dispatch("place.pick", &pick),
        Outcome::Done
    );
    settle(&mut h);
    let reps = |h: &Harness<'_, App>| h.state().shell().canvas().unwrap().page().tables.reps.len();
    assert!(h.state().shell().is_dirty());

    // File > Open, Discard: the dialog is open with `discard` already given.
    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    h.get_by_label_contains("Open…").click();
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(ew_ui::ew_viewmodel::shell::Dialog::ConfirmDiscard { .. })
    ));
    h.get_by_label("Don't save").click();
    h.step();
    h.step();
    assert_eq!(platform.started.get(), 1, "the dialog is open");
    assert!(h.state().shell().dialog().is_none());

    let revision = h.state().shell().revision();
    let before = reps(&h);
    let painted = h.ctx.cumulative_pass_nr();
    // A key that edits (undo), a key that opens the palette, a menu action that edits, a click that places.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.step();
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    h.step();
    support::open_menu(&mut h, &["Edit"]);
    h.step();
    h.step();
    h.get_all_by_label_contains("Undo")
        .next()
        .expect("the Edit menu's Undo")
        .click();
    h.step();
    h.step();
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    let at = screen(&h, (w * 3 / 4, hh * 3 / 4));
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    let shell = h.state().shell();
    assert_eq!(shell.revision(), revision, "the model is untouched");
    assert!(shell.is_dirty());
    assert_eq!(reps(&h), before, "the click placed nothing");
    assert!(shell.palette().is_none(), "no shell action ran");
    assert!(shell.dialog().is_none());
    assert_eq!(shell.tool().name(), "place");
    assert!(
        shell.canvas().unwrap().pointer().is_none(),
        "the pointer did not reach the shell"
    );
    assert!(h.ctx.cumulative_pass_nr() > painted, "the window kept painting");
    assert_eq!(platform.started.get(), 1, "still the one dialog");

    // The answer proceeds with the open.
    let tx = platform.answer.borrow_mut().take().unwrap();
    tx.send(Pick::Path(other)).unwrap();
    h.step();
    assert_eq!(h.state().shell().project_name(), Some("h.ew"));
    assert!(!h.state().shell().is_dirty());
    h.state_mut().shell_mut().shutdown();
}

/// FT-467: `spawn_pick` puts the answer in the channel before it requests the repaint, so the frame the request starts
/// finds it.
#[test]
fn spawn_pick_sends_the_answer_before_it_requests_a_repaint() {
    let ctx = egui::Context::default();
    let (woke_tx, woke_rx) = std::sync::mpsc::channel::<()>();
    let woke_tx = std::sync::Mutex::new(woke_tx);
    ctx.set_request_repaint_callback(move |_| {
        let _ = woke_tx.lock().unwrap().send(());
    });
    let answer = ew_ui::app::spawn_pick(&ctx, || Pick::Path(PathBuf::from("a.csv"))).unwrap();
    woke_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the dialog thread requested a repaint");
    assert_eq!(answer.try_recv(), Ok(Pick::Path(PathBuf::from("a.csv"))));
}
