// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… of a parts table in the window without rendering: it goes on to Parts (doc 07 §3.6) with or without
//! a project; with a new project it raises the file prompt (`Prompt::OpenImport`), the answer shows the table's set in
//! Parts with which column is what as its first step, and its Save writes the parts into My parts; Discard drops it;
//! the accessible-name walker after the steps. The file dialog's frames and its thread.

use crate::support;

use std::path::{Path, PathBuf};

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use egui::{Key, Modifiers, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::{App, Pick, PickReply, Platform, setup};
use ew_ui::clock::Clock;
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, Outcome, Prompt, Screen, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, screen, settle, wait_for_page};

/// The newest notification's text: the toast it showed (the harness's clock may already have timed the toast out).
fn toast(h: &Harness<'_, App>) -> Option<String> {
    h.state()
        .shell()
        .notifications()
        .entries()
        .next()
        .map(|n| n.text.clone())
}

/// A shell whose person has a user folder under `tmp` (My parts is made there on the first Save).
fn shell(tmp: &Tmp) -> ShellModel {
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    ShellModel::new(Mode::Gui, tmp.config().with_libraries(Some(mine)))
}

/// A shell on a new project with the built-in template, and the project's directory.
fn new_project(tmp: &Tmp) -> (ShellModel, PathBuf) {
    let dir = tmp.0.join("proj.ew");
    let mut shell = shell(tmp);
    let params: Value = json!({"dir": dir.to_string_lossy(), "template": "builtin"});
    assert_eq!(shell.dispatch("file.new", &params), Outcome::Done);
    (shell, dir)
}

fn table_parts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tables/parts.csv")
}

/// A table of two parts in the project tables' columns.
fn two_parts(tmp: &Tmp) -> PathBuf {
    let csv = tmp.0.join("parts.csv");
    std::fs::write(
        &csv,
        "key,manufacturer,part_number,kind,type_designation,description,unit,test_data,tech_data\nA1,Acme,A1,,,Relay A,piece,,\nA2,Acme,A2,,,Relay B,piece,,\n",
    )
    .unwrap();
    csv
}

#[test]
fn without_a_project_a_parts_table_goes_on_to_parts() {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(table_parts()));
    let mut h = harness_with(shell(&tmp), platform, None);
    settle(&mut h);
    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    h.get_by_label("Import…").click();
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().screen(), Screen::Parts);
    assert!(h.state().shell().import_set().is_some(), "the table's set");
    assert!(h.state().shell().parts_without_project());
    support::assert_accessible(&h);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_entry_prompts_and_save_writes_the_set_into_my_parts() {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(table_parts()));
    let (shell, _) = new_project(&tmp);
    let mut h = harness_with(shell, platform.clone(), None);
    settle(&mut h);
    wait_for_page(&mut h);

    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    assert!(!h.get_by_label("Import…").accesskit_node().is_disabled());
    h.get_by_label("Import…").click();
    settle(&mut h);
    settle(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::OpenImport]);
    // Parts shows the set, which column is what first, a drop-down per column.
    assert_eq!(h.state().shell().screen(), Screen::Parts);
    assert!(h.query_by_label("Which column is what").is_some(), "the mapping");
    assert!(
        h.query_by_label("Goes to part_number").is_some(),
        "a field drop-down per column"
    );
    support::assert_accessible(&h);
    // Every row of a maker and a number is a part; the rows that repeat one with other values are refused.
    let new = h.state().shell().import_set().map_or(0, |s| s.to_save());
    assert!(new > 0);
    h.get_by_label(&format!("Save {new} to My parts")).click();
    settle(&mut h);
    assert!(h.state().shell().import_set().is_none(), "saved");
    assert_eq!(
        toast(&h),
        Some(format!(
            "parts.csv: {new} new parts and 0 changed ones saved to My parts."
        ))
    );
    assert!(!h.state().shell().can_undo(), "nothing went into the project");
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn discard_drops_the_set_without_a_change() {
    let tmp = Tmp::new();
    let (shell, _) = new_project(&tmp);
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    wait_for_page(&mut h);
    let path = table_parts();
    let out = h
        .state_mut()
        .shell_mut()
        .dispatch("parts.import", &json!({"path": path.to_string_lossy()}));
    assert_eq!(out, Outcome::Done);
    settle(&mut h);
    h.get_by_label("Discard").click();
    settle(&mut h);
    assert!(h.state().shell().import_set().is_none());
    assert!(
        !tmp.0
            .join("data")
            .join("libraries")
            .join("user-parts.ewlib")
            .exists()
    );
    h.state_mut().shell_mut().shutdown();
}

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

/// The frame that answers the file dialog, with no pointer input, has the menu closed and the set in the shell; the
/// following frame paints it, and Save's next frames say it.
#[test]
fn the_set_and_the_closed_menu_arrive_in_the_frame_after_the_pick() {
    let tmp = Tmp::new();
    let csv = two_parts(&tmp);
    let (shell, _) = new_project(&tmp);
    let ctx_slot = Rc::new(RefCell::new(None));
    let pick_pass = Rc::new(Cell::new(u64::MAX));
    let platform = DialogPlatform {
        path: csv,
        ctx: Rc::clone(&ctx_slot),
        pass: Rc::clone(&pick_pass),
    };
    let app = App::new(shell, Box::new(platform), None, Clock::Wall);
    let mut h = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .with_context_setup(setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    *ctx_slot.borrow_mut() = Some(h.ctx.clone());
    settle(&mut h);

    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    h.get_by_label("Import…").click();
    // Step frames until the one in which the dialog was answered; it must already show the menu closed and the set in
    // the shell (the set is laid out in the following frame).
    let mut answered = false;
    for _ in 0..6 {
        let before = h.ctx.cumulative_pass_nr();
        h.step();
        let after = h.ctx.cumulative_pass_nr();
        let at = pick_pass.get();
        if at != u64::MAX && before <= at && at <= after {
            assert!(
                h.query_by_label("Import…").is_none(),
                "the File menu closes in the frame that answers the pick"
            );
            assert!(
                h.state().shell().import_set().is_some(),
                "the set is in the shell in the frame that answers the pick"
            );
            answered = true;
            break;
        }
    }
    assert!(answered, "the pick was answered");
    h.step();
    assert!(
        h.query_by_label("Which column is what").is_some(),
        "the set is painted"
    );
    h.get_by_label("Save 2 to My parts").click();
    h.step();
    h.step();
    assert_eq!(
        toast(&h).as_deref(),
        Some("parts.csv: 2 new parts and 0 changed ones saved to My parts.")
    );
    h.state_mut().shell_mut().shutdown();
}

/// A platform whose dialog stays open off the window's thread until the test answers it through the sender.
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

/// A harness on a new project whose File ▸ Import… has been clicked once with `platform`.
fn harness_with_open_dialog<'a>(tmp: &Tmp, platform: ThreadedPlatform) -> Harness<'a, App> {
    let (shell, _) = new_project(tmp);
    let app = App::new(shell, Box::new(platform), None, Clock::Wall);
    let mut h = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .with_context_setup(setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    settle(&mut h);
    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    h.get_by_label("Import…").click();
    h.step();
    h.step();
    h
}

/// While the dialog is open on its own thread the window keeps painting, nothing is dispatched and no second
/// dialog starts; the frame after the answer arrives has the set in the shell, and the next one paints it.
#[test]
fn an_answer_from_the_dialog_thread_shows_the_set_in_the_next_frame() {
    let tmp = Tmp::new();
    let csv = two_parts(&tmp);
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
    assert!(h.state().shell().import_set().is_none(), "no set before the answer");
    assert_eq!(platform.started.get(), 1, "no second dialog while one is open");
    assert!(
        h.query_by_label("Import…").is_none(),
        "the File menu is closed while the dialog is open"
    );

    let tx = platform.answer.borrow_mut().take().unwrap();
    tx.send(Pick::Path(csv)).unwrap();
    h.step();
    assert!(
        h.state().shell().import_set().is_some(),
        "the set is in the shell in the frame after the answer"
    );
    h.step();
    assert!(
        h.query_by_label("Which column is what").is_some(),
        "the set is painted"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A dialog thread that ends without an answer (it panicked) counts as a cancel: no set, no change, and the
/// next prompt starts a new dialog.
#[test]
fn a_dialog_thread_without_an_answer_counts_as_cancelled() {
    let tmp = Tmp::new();
    let platform = ThreadedPlatform::default();
    let mut h = harness_with_open_dialog(&tmp, platform.clone());
    drop(platform.answer.borrow_mut().take());
    h.step();
    h.step();
    assert!(h.state().shell().import_set().is_none());
    assert!(!h.state().shell().can_undo());
    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    h.get_by_label("Import…").click();
    h.step();
    h.step();
    assert_eq!(platform.started.get(), 2, "a new prompt starts a new dialog");
    h.state_mut().shell_mut().shutdown();
}

/// While a file dialog is open off the window's thread no input reaches the shell (a dialog that is not
/// modal, as on Linux, would otherwise let edits be made that the answer's `discard` throws away unasked): a key, a
/// menu action and a canvas click change nothing and dispatch nothing, the window keeps painting, and the answer then
/// proceeds with the open.
#[test]
fn no_input_reaches_the_shell_while_a_file_dialog_is_open() {
    let tmp = Tmp::new();
    let dir = tmp.sample("s.ew");
    let other = tmp.sample("h.ew");
    let platform = ThreadedPlatform::default();
    let shell = ShellModel::new(Mode::Gui, tmp.config());
    let app = App::new(shell, Box::new(platform.clone()), Some(dir), Clock::Wall);
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
        Some(ew_viewmodel::shell::Dialog::ConfirmDiscard { .. })
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

/// `spawn_pick` puts the answer in the channel before it requests the repaint, so the frame the request starts
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
