// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawing of a symbol in Parts (doc 07 §5.6): its tools, its selection and Properties, its history, Save into My
//! parts with a key from its name, Test on a page, Export, and the questions before a drawing with changes goes.

use crate::common::{Tmp, done, failed, new_drawing};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

/// A one-pin drawing named `name`, a new symbol of his own in Parts.
fn drawn(shell: &mut ShellModel, name: &str) {
    new_drawing(shell, name);
    done(
        shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4}),
    );
    done(shell, "symbol.cp", json!({"x":0,"y":0,"designation":"1"}));
}

/// Saves the drawing with his parts and leaves Parts; the key the symbol was saved under.
fn saved_and_left(shell: &mut ShellModel) -> String {
    done(shell, "parts.save", json!({}));
    let key = shell.symbol_editor().expect("the drawing").draft.key.clone();
    done(shell, "parts.back", json!({}));
    done(shell, "parts.back", json!({}));
    assert!(shell.parts_place().is_none(), "back on the page");
    key
}

/// The key of every symbol of the project saved at `dir` that starts with `start`.
fn project_keys(dir: &std::path::Path, start: &str) -> Vec<String> {
    let (session, _) = ew_api::Session::open_read_only(dir, "ElektraWatt test").unwrap();
    let q = session.query();
    q.symbols()
        .iter()
        .filter_map(|(_, s)| s.item.meta.key.as_ref().map(ToString::to_string))
        .filter(|k| k.starts_with(start))
        .collect()
}

/// A symbol saved in Parts is offered by the symbol picker as ordinary geometry and placed; a test copy keeps an
/// identity of its own in the project; a fresh window finds the symbol on the Symbols shelf among his own.
#[test]
fn a_saved_symbol_is_offered_by_the_picker_and_a_test_copy_keeps_its_own_identity() {
    let tmp = Tmp::new("symbol-publish-insert");
    let dir = tmp.0.join("project");
    // His user folder, where My parts lives.
    let config = || {
        let mine = ew_api::library::LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), vec![], vec![]);
        tmp.config().with_libraries(Some(mine))
    };
    let mut shell = ShellModel::new(Mode::Headless, config());
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    done(
        &mut shell,
        "command.run",
        json!({"cmd":"page.create","params":{"number":2,"grid":{"num":4000000,"den":1}}}),
    );
    done(&mut shell, "page.goto", json!({"folio":"2"}));
    done(&mut shell, "tool.place", json!({})); // Prime the catalogue before the save.
    done(&mut shell, "ui.dismiss", json!({}));
    drawn(&mut shell, "Published contact");
    let key = saved_and_left(&mut shell);
    done(&mut shell, "tool.place", json!({}));
    let row = shell
        .picker()
        .unwrap()
        .catalogue
        .symbols
        .iter()
        .find(|r| r.name.starts_with("Published contact"))
        .unwrap_or_else(|| panic!("{:?}", shell.picker().unwrap().catalogue))
        .clone();
    assert!(row.key.starts_with("library:"));
    assert!(!row.black_box, "drawn geometry is placed as a symbol");
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "place.pick", json!({"symbol":row.key,"letter":"U"}));
    done(&mut shell, "cursor.set", json!({"x_mm":40,"y_mm":40}));
    done(&mut shell, "canvas.activate", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    // Opened again from the picker's address, changed and tested on the page.
    done(&mut shell, "parts.open", json!({"symbol": row.key}));
    assert!(
        shell.symbol_editor().is_some_and(|e| e.locked.is_none()),
        "his own symbol opens to be changed"
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":4,"y":0,"x2":4,"y2":4}),
    );
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let keys = project_keys(&dir, &key);
    assert!(keys.contains(&key), "{keys:?}");
    assert!(
        keys.iter().any(|k| k.starts_with(&format!("{key}.test."))),
        "{keys:?}"
    );
    // A fresh window lists it on the Symbols shelf, among his own.
    let mut reopened = ShellModel::new(Mode::Headless, config());
    done(&mut reopened, "parts.open", json!({}));
    done(
        &mut reopened,
        "parts.find",
        json!({"shelf": "symbols", "scope": "mine", "text": "published"}),
    );
    let place = reopened.parts_place().unwrap();
    assert!(
        place
            .symbols
            .iter()
            .any(|s| s.name == "Published contact" && s.mine),
        "{:?}",
        place.symbols
    );
    reopened.shutdown();
}

/// A new symbol drawn for a page on the QET grid is drawn on that grid; saved and placed there it is ordinary
/// geometry, and a page on the 4 mm grid does not offer it.
#[test]
fn a_symbol_drawn_for_a_qet_page_keeps_its_grid_after_placing() {
    let tmp = Tmp::new("symbol-qet-catalogue");
    let dir = tmp.0.join("project");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    for (number, grid) in [(2, json!({"num":7937500,"den":3})), (3, json!({"num":4000000,"den":1}))] {
        done(
            &mut shell,
            "command.run",
            json!({"cmd":"page.create","params":{"number":number,"grid":grid}}),
        );
    }
    done(&mut shell, "page.goto", json!({"folio":"2"}));
    done(&mut shell, "parts.new_symbol", json!({}));
    let grid = serde_json::to_value(&shell.symbol_editor().unwrap().draft.symbol.grid).unwrap();
    assert_eq!(grid, json!({"num":7937500,"den":3}), "page 2's grid");
    done(
        &mut shell,
        "parts.field",
        json!({"field":"symbol.name","value":"QET graphic"}),
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":4,"y2":0}),
    );
    // Save and place on 2: the page shows with the Place tool armed with it.
    done(&mut shell, "parts.save", json!({}));
    assert!(shell.parts_place().is_none());
    assert_eq!(shell.tool().name(), "place");
    done(&mut shell, "cursor.set", json!({"x_mm":40,"y_mm":40}));
    done(&mut shell, "canvas.activate", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = ew_api::Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    let offered = |folio: &str| {
        session
            .place_catalogue(folio)
            .unwrap()
            .symbols
            .iter()
            .filter(|r| r.name.starts_with("QET graphic"))
            .map(|r| r.black_box)
            .collect::<Vec<_>>()
    };
    assert!(
        !offered("2").is_empty() && offered("2").iter().all(|b| !b),
        "page 2 offers it as a symbol"
    );
    assert!(offered("3").is_empty(), "page 3 is on another grid");
}

/// The drawing's history, Export, Save, Test on a page and the drawing in hand, in both frontends: a point off the
/// grid is said in words; Undo and Redo step; a symbol of another grid is refused on the page in front and tested on
/// a page of its own grid; Back keeps the drawing with its changes, which an import over it asks for.
#[test]
fn a_drawings_history_export_save_and_test_in_both_frontends() {
    for mode in [Mode::Headless, Mode::Gui] {
        let tmp = Tmp::new("symbol-editor");
        let mut shell = tmp.empty_shell(mode);
        done(
            &mut shell,
            "file.new",
            json!({"dir":tmp.0.join("scratch.ew"),"template":"builtin"}),
        );
        done(&mut shell, "page.goto", json!({"folio":"1"}));
        new_drawing(&mut shell, "Power block");
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool":"rectangle","x":0.5,"y":0,"x2":24,"y2":28}),
        );
        for i in 1..=6 {
            done(
                &mut shell,
                "symbol.cp",
                json!({"x":0,"y":i*4,"designation":if i==6{"FE".into()}else{i.to_string()},"direction":"left"}),
            );
        }
        done(&mut shell, "symbol.validate", json!({}));
        let original = shell.symbol_editor().unwrap().draft.clone();
        done(&mut shell, "symbol.cp", json!({"idx":1,"x":4.5,"y":4}));
        assert_eq!(
            failed(&mut shell, "symbol.validate", json!({})),
            "Connection point 1 is off the connection grid. Move it onto the grid before saving."
        );
        done(&mut shell, "edit.undo", json!({}));
        assert_eq!(shell.symbol_editor().unwrap().draft, original);
        done(&mut shell, "edit.redo", json!({}));
        let first = shell.symbol_editor().unwrap().draft.symbol.cps.iter().next().unwrap().pos;
        assert_eq!(first.x.0, 4_500_000);
        done(&mut shell, "edit.undo", json!({}));
        let file = tmp.0.join("power.symbol.json");
        done(&mut shell, "symbol.export", json!({"file":file}));
        done(&mut shell, "parts.save", json!({}));
        done(&mut shell, "symbol.variant", json!({"variant":7}));
        // Page 1 is on the QET grid: the 4 mm symbol is refused there in words, both grids named.
        let pages = shell.page_count();
        assert_eq!(
            failed(&mut shell, "symbol.place", json!({"x":80,"y":80})),
            "This symbol is drawn on the 4 mm connection grid, and this page uses QET 10 px. Place it on a page with \
             its grid."
        );
        assert_eq!(shell.page_count(), pages);
        // Test on a page: a page of its grid shows; the drawing waits in hand.
        done(&mut shell, "symbol.scratch", json!({}));
        assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (4_000_000, 1));
        assert!(shell.symbol_editor().is_none());
        done(&mut shell, "parts.new_symbol", json!({"like": "hand"}));
        done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
        done(&mut shell, "parts.new_symbol", json!({"like": "hand"}));
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool":"line","x":1,"y":1,"x2":2,"y2":2}),
        );
        // Back keeps the drawing with its changes; an import over it asks first.
        let draft = shell.symbol_editor().unwrap().draft.clone();
        done(&mut shell, "parts.back", json!({}));
        assert!(shell.symbol_editor().is_none());
        assert_eq!(shell.kept_symbol_editor().unwrap().draft, draft);
        if mode == Mode::Headless {
            assert_eq!(
                failed(&mut shell, "symbol.open", json!({"file":file})),
                "This symbol has unsaved changes. Save it or choose Discard changes."
            );
        }
        done(&mut shell, "symbol.open", json!({"file":file,"discard":true}));
        assert_eq!(
            shell.symbol_editor().unwrap().draft.symbol,
            original.symbol,
            "the exported drawing"
        );
        shell.shutdown();
    }
}

/// Each Test on a page takes a free folio of its own.
#[test]
fn test_on_a_page_takes_a_free_folio_each_time() {
    let tmp = Tmp::new("symbol-scratch");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("scratch.ew"),"template":"builtin"}),
    );
    drawn(&mut shell, "Contact");
    let pages = shell.page_count();
    done(&mut shell, "symbol.scratch", json!({}));
    assert_eq!(shell.page_count(), pages + 1);
    done(&mut shell, "parts.new_symbol", json!({"like": "hand"}));
    done(&mut shell, "symbol.scratch", json!({}));
    assert_eq!(shell.page_count(), pages + 2);
    shell.shutdown();
}

#[test]
fn project_save_confirmation_saves_the_project_with_a_symbol_open() {
    use ew_viewmodel::shell::ConfirmAnswer;
    for pending in ["file.close", "app.quit"] {
        let tmp = Tmp::new("symbol-project-confirm");
        let mut shell = tmp.empty_shell(Mode::Gui);
        done(
            &mut shell,
            "file.new",
            json!({"dir":tmp.0.join("project.ew"),"template":"builtin"}),
        );
        done(&mut shell, "page.new", json!({"folio":"42"}));
        new_drawing(&mut shell, "Test");
        done(
            &mut shell,
            "symbol.export",
            json!({"file":tmp.0.join("draft.json")}),
        );
        assert!(shell.is_dirty());
        done(&mut shell, pending, json!({}));
        assert_eq!(shell.answer_confirm(ConfirmAnswer::Save), Outcome::Done);
        assert!(!shell.has_project(), "{pending}");
        shell.shutdown();
    }
}

/// A person names a symbol and never gives it a key: the key is made from the name and follows it; Save saves into
/// My parts and says so, and a second symbol of the same name takes the next free key instead of a refusal. An
/// import takes its file's name.
#[test]
fn a_new_symbol_takes_its_key_from_its_name() {
    let tmp = Tmp::new("symbol-key-from-name");
    let mut shell = tmp.empty_shell(Mode::Headless);
    let key = |shell: &ShellModel| shell.symbol_editor().unwrap().draft.key.clone();
    let name = |shell: &mut ShellModel, name: &str| {
        done(
            shell,
            "parts.field",
            json!({"field":"symbol.name","value":name}),
        );
    };
    done(&mut shell, "parts.open", json!({}));
    done(&mut shell, "parts.new_symbol", json!({}));
    assert!(!shell.symbol_editor().unwrap().dirty(), "a new drawing is no change");
    let line = json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4});
    done(&mut shell, "symbol.draw", line.clone());
    name(&mut shell, "Pump 3 (spare)");
    assert_eq!(key(&shell), "user.pump-3-spare");
    name(&mut shell, "Pump station");
    assert_eq!(key(&shell), "user.pump-station");
    done(&mut shell, "parts.save", json!({}));
    assert_eq!(
        shell.toast().map(|t| t.text.clone()).as_deref(),
        Some("Saved Pump station to the library My parts. Draw ▸ Symbol places it on a page.")
    );
    let first = shell.symbol_editor().unwrap().draft.identity.clone();
    // A second new symbol of the same name is a second symbol.
    new_drawing(&mut shell, "Pump station");
    done(&mut shell, "symbol.draw", line);
    assert_eq!(key(&shell), "user.pump-station");
    done(&mut shell, "parts.save", json!({}));
    assert_eq!(key(&shell), "user.pump-station-2");
    assert!(!shell.symbol_editor().unwrap().dirty());
    assert_ne!(shell.symbol_editor().unwrap().draft.identity, first);
    // An import without a key takes its file's name.
    let file = tmp.0.join("contact no.elmt");
    std::fs::write(
        &file,
        include_bytes!("../../../ew-import-qet/tests/fixtures/contact_no.elmt"),
    )
    .unwrap();
    done(&mut shell, "symbol.open", json!({"file":file,"discard":true}));
    assert_eq!(key(&shell), "contact_no");
    shell.shutdown();
}

/// Without configured libraries a symbol is saved into My parts of the settings folder, its index there and not
/// beside the library; a library of the user tier is not remembered as a company library.
#[test]
fn saving_without_configured_tiers_keeps_the_index_in_the_settings_folder() {
    let tmp = Tmp::new("symbol-tiers");
    let config = tmp.0.join("config");
    let mut shell = ShellModel::new(Mode::Headless, ShellConfig::in_dir(&config, "0.0.0-test"));
    drawn(&mut shell, "Tiers");
    done(&mut shell, "parts.save", json!({}));
    assert!(config.join("library-index").is_dir());
    shell.shutdown();
    let data = tmp.0.join("data");
    let tiers = ew_api::library::LibraryTiers::under(&data, &tmp.0.join("cache"), vec![], vec![]);
    let user = tmp.0.join("user-config");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&user, "0.0.0-test").with_libraries(Some(tiers)),
    );
    drawn(&mut shell, "User");
    done(&mut shell, "parts.save", json!({}));
    shell.shutdown();
    let settings = std::fs::read_to_string(user.join("settings.toml")).unwrap_or_default();
    assert!(!settings.contains("company"), "{settings}");
}

#[test]
fn an_export_over_a_file_is_refused_in_words() {
    let tmp = Tmp::new("symbol-refusals");
    let mut shell = tmp.empty_shell(Mode::Headless);
    drawn(&mut shell, "Words");
    let file = json!({"file":tmp.0.join("words.symbol.json")});
    done(&mut shell, "symbol.export", file.clone());
    assert_eq!(
        failed(&mut shell, "symbol.export", file),
        "A file with this name already exists. Choose another name."
    );
    shell.shutdown();
}

/// Back leaves the drawing and keeps it with its history; New symbol ▸ Go on with the drawing in hand returns to it;
/// a new drawing with `discard` drops it.
#[test]
fn back_keeps_the_drawing_and_its_history_until_discarded() {
    let tmp = Tmp::new("symbol-keep");
    let mut shell = tmp.empty_shell(Mode::Headless);
    drawn(&mut shell, "Kept");
    let draft = shell.symbol_editor().unwrap().draft.clone();
    done(&mut shell, "parts.back", json!({}));
    assert!(shell.symbol_editor().is_none());
    assert!(shell.kept_symbol_editor().unwrap().dirty());
    assert_eq!(
        failed(&mut shell, "parts.new_symbol", json!({})),
        "This symbol has unsaved changes. Save it or choose Discard changes."
    );
    done(&mut shell, "parts.new_symbol", json!({"like": "hand"}));
    let editor = shell.symbol_editor().unwrap();
    assert_eq!(editor.draft, draft);
    assert!(editor.can_undo());
    done(&mut shell, "edit.undo", json!({}));
    assert!(shell.symbol_editor().unwrap().draft.symbol.cps.is_empty());
    done(&mut shell, "parts.new_symbol", json!({"discard": true}));
    let editor = shell.symbol_editor().unwrap();
    assert!(editor.draft.symbol.graphics.is_empty());
    assert!(!editor.dirty() && !editor.can_undo());
    shell.shutdown();
}

/// An unedited new or imported drawing is no unsaved change and is not saved yet; an exported one is saved.
#[test]
fn the_drawing_state_says_unsaved_only_after_an_edit_and_saved_only_once_stored() {
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-state");
    let mut shell = tmp.empty_shell(Mode::Headless);
    let words = |shell: &ShellModel| strings::text(shell.symbol_editor().unwrap().state_key()).to_owned();
    done(&mut shell, "parts.open", json!({}));
    done(&mut shell, "parts.new_symbol", json!({}));
    assert!(!shell.symbol_editor().unwrap().dirty());
    assert_eq!(words(&shell), "Not saved yet");
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4}),
    );
    assert_eq!(words(&shell), "Unsaved changes");
    let file = tmp.0.join("state.symbol.json");
    done(&mut shell, "symbol.export", json!({"file":file}));
    assert_eq!(words(&shell), "Saved");
    done(&mut shell, "symbol.open", json!({"file":file,"discard":true}));
    assert!(!shell.symbol_editor().unwrap().dirty());
    assert_eq!(words(&shell), "Not saved yet");
    let qet = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ew-import-qet/tests/fixtures/contact_no.elmt"
    );
    done(&mut shell, "symbol.open", json!({"file":qet}));
    assert_eq!(words(&shell), "Not saved yet");
    shell.shutdown();
}

/// An empty path is refused in words before any file system call, and a relative one resolves against the user
/// library folder, never the working directory.
#[test]
fn symbol_paths_are_refused_when_empty_and_resolve_against_the_user_library_folder() {
    let tmp = Tmp::new("symbol-paths");
    let config = tmp.0.join("config");
    let mut shell = ShellModel::new(Mode::Headless, ShellConfig::in_dir(&config, "0.0.0-test"));
    assert_eq!(
        failed(&mut shell, "symbol.open", json!({"file":""})),
        "Choose a symbol file first."
    );
    drawn(&mut shell, "Paths");
    assert_eq!(
        failed(&mut shell, "symbol.export", json!({"file":"  "})),
        "Choose a symbol file first."
    );
    done(&mut shell, "parts.save", json!({}));
    done(&mut shell, "symbol.export", json!({"file":"paths.symbol.json"}));
    assert!(config.join("libraries").join("paths.symbol.json").is_file());
    shell.shutdown();
}

/// In the window a missing path asks the house picker.
#[test]
fn the_window_asks_the_pickers_for_a_missing_symbol_path() {
    use ew_viewmodel::shell::Prompt;
    let tmp = Tmp::new("symbol-pickers");
    let mut shell = tmp.empty_shell(Mode::Gui);
    new_drawing(&mut shell, "Pick");
    for (action, prompt) in [
        ("symbol.open", Prompt::OpenSymbol),
        ("symbol.export", Prompt::SaveSymbol),
    ] {
        done(&mut shell, action, json!({}));
        let asked = shell.take_prompt().unwrap();
        assert_eq!((asked.action, asked.prompt), (action, prompt));
    }
    shell.shutdown();
}

/// A test placement shows the page with the symbol selected and offers the way back to the drawing in hand.
#[test]
fn a_test_placement_shows_the_page_and_its_toast_returns_to_the_drawing() {
    let tmp = Tmp::new("symbol-back");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project.ew"),"template":"builtin"}),
    );
    drawn(&mut shell, "Back");
    done(&mut shell, "symbol.scratch", json!({}));
    assert!(shell.symbol_editor().is_none());
    assert_eq!(shell.selection().items.len(), 1);
    let toast = shell.notifications().newest_visible().unwrap().clone();
    let page = shell.page_labels()[shell.page_index()].clone();
    assert_eq!(toast.text, format!("Test copy placed on page {page}."));
    assert_eq!(toast.action(), Some("edit"));
    done(
        &mut shell,
        "notifications.run",
        json!({"entry": toast.serial, "action": "edit"}),
    );
    assert_eq!(shell.symbol_editor().unwrap().draft.key, "user.back");
    done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
    assert_eq!(
        shell.notifications().newest_visible().unwrap().text,
        "Test copy placed on this page."
    );
    shell.shutdown();
}

/// The test copy's key is derived from its content, so placing again after a reopen embeds a new copy.
#[test]
fn a_test_copy_placed_again_after_reopening_the_project_takes_its_own_key() {
    let tmp = Tmp::new("symbol-reopen");
    let config = tmp.config();
    let dir = tmp.0.join("project.ew");
    let mut shell = ShellModel::new(Mode::Headless, config.clone());
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "Reopen");
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let mut shell = ShellModel::new(Mode::Headless, config);
    done(&mut shell, "file.open", json!({"dir":dir}));
    drawn(&mut shell, "Reopen");
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":4,"y":0,"x2":4,"y2":4}),
    );
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let keys: std::collections::BTreeSet<String> =
        project_keys(&dir, "user.reopen.test.").into_iter().collect();
    assert_eq!(keys.len(), 2, "{keys:?}");
}

/// Quitting with a drawing with changes asks Save symbol, Discard or Cancel; `discard` discards it.
#[test]
fn quitting_with_an_unsaved_drawing_asks_and_discard_discards_it() {
    use ew_viewmodel::dialogs::spec;
    use ew_viewmodel::shell::{ConfirmAnswer, Dialog};
    let tmp = Tmp::new("symbol-quit");
    let config = tmp.config();
    let mut shell = ShellModel::new(Mode::Headless, config.clone());
    drawn(&mut shell, "Quit block");
    done(&mut shell, "parts.back", json!({}));
    assert_eq!(
        failed(&mut shell, "app.quit", json!({})),
        "This symbol has unsaved changes. Save it or choose Discard changes."
    );
    done(&mut shell, "app.quit", json!({"discard": true}));
    assert!(shell.should_quit() && shell.kept_symbol_editor().is_none());
    let mut shell = ShellModel::new(Mode::Gui, config);
    drawn(&mut shell, "Quit block");
    done(&mut shell, "app.quit", json!({}));
    let dialog = shell.dialog().unwrap().clone();
    assert!(matches!(dialog, Dialog::ConfirmDiscard { symbol: true, .. }));
    let s = spec(&dialog, &shell);
    assert_eq!(s.title, "Save the symbol Quit block?");
    let labels: Vec<&str> = s.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Save symbol", "Cancel", "Discard changes"]);
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Cancel), Outcome::Done);
    assert!(!shell.should_quit() && shell.symbol_editor().is_some());
    done(&mut shell, "app.quit", json!({}));
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Discard), Outcome::Done);
    assert!(shell.should_quit() && shell.kept_symbol_editor().is_none());
    shell.shutdown();
}

/// While the drawing shows, Ctrl+S saves the symbol and the File menu names Save symbol and keeps Save project.
#[test]
fn the_file_menu_names_save_symbol_and_keeps_save_project_while_the_drawing_shows() {
    use ew_viewmodel::actions::{MenuEntry, menu_model};
    let tmp = Tmp::new("symbol-menu");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project.ew"),"template":"builtin"}),
    );
    let labels = |shell: &ShellModel| -> Vec<String> {
        menu_model(shell)
            .into_iter()
            .find(|m| m.label == "File")
            .unwrap()
            .entries
            .into_iter()
            .filter_map(|e| match e {
                MenuEntry::Item(i) if i.id.starts_with("file.save") => Some(i.label),
                _ => None,
            })
            .collect()
    };
    assert_eq!(labels(&shell), ["Save", "Save as…"]);
    drawn(&mut shell, "Menu");
    assert_eq!(labels(&shell), ["Save symbol", "Save project", "Save as…"]);
    done(&mut shell, "page.new", json!({"folio":"7"}));
    assert!(shell.is_dirty());
    done(&mut shell, "file.save_project", json!({}));
    assert!(!shell.is_dirty());
    done(&mut shell, "file.save", json!({}));
    assert!(shell.symbol_editor().unwrap().stored());
    shell.shutdown();
}

/// The file picker offers exactly the extensions the drawing reads: JSON, and every extension an importer takes (a
/// file the JSON reader would refuse otherwise), `.pxf` among them.
#[test]
fn the_symbol_file_picker_offers_every_extension_an_importer_reads() {
    use ew_api::symbols;
    use ew_viewmodel::shell::symbol_editor::FILE_EXTENSIONS;
    let tmp = Tmp::new("symbol-extensions");
    let refusal = |ext: &str| {
        let path = tmp.0.join(format!("probe.{ext}"));
        std::fs::write(&path, b"not a symbol").unwrap();
        symbols::open(&path, "example.probe").unwrap_err().to_string()
    };
    let json = refusal("json");
    let read: Vec<&str> = [
        "elmt", "edz", "pxf", "ema", "ems", "esl", "slk", "xml", "zip", "7z", "qet", "txt", "dxf",
    ]
    .into_iter()
    .filter(|ext| refusal(ext) != json)
    .collect();
    let offered: Vec<&str> = FILE_EXTENSIONS.into_iter().filter(|e| *e != "json").collect();
    assert_eq!(offered, read);
    assert!(FILE_EXTENSIONS.contains(&"json"));
}

/// On a QET-grid page the Insert symbol picker lists a saved 4 mm symbol with both grids in words and what Place
/// does: it is placed on the page in front as its copy drawn on the page's grid, every connection point on a grid
/// point of the page, so wires meet it.
#[test]
fn a_symbol_on_another_grid_is_placed_on_the_page_in_front_on_its_grid() {
    use ew_viewmodel::strings;
    use ew_viewmodel::tools::PickerRow;
    let tmp = Tmp::new("symbol-picker-grid");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project.ew"),"template":"builtin"}),
    );
    assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (7_937_500, 3));
    drawn(&mut shell, "Grid block");
    // A second point two 4 mm steps on: 8 mm is no point of the QET grid.
    done(
        &mut shell,
        "symbol.cp",
        json!({"x": 8, "y": 0, "designation": "2"}),
    );
    saved_and_left(&mut shell);
    done(&mut shell, "tool.place", json!({}));
    let picker = shell.picker().unwrap();
    let i = picker
        .catalogue
        .symbols
        .iter()
        .position(|s| s.key.starts_with("library:") && s.name.starts_with("Grid block"))
        .expect("the saved symbol is listed");
    let row = PickerRow::Symbol(i);
    assert!(picker.rows.contains(&row));
    let words = "This symbol is drawn on the 4 mm connection grid, and this page uses QET 10 px. Place puts it here \
                 with its connection points on this page's grid.";
    assert_eq!(picker.disabled_words(row), None, "the row can be placed");
    assert_eq!(picker.grid_note(row).as_deref(), Some(words));
    assert!(strings::set_locale("nl"));
    let dutch = shell.picker().unwrap().grid_note(row);
    assert!(strings::set_locale("en"));
    assert_eq!(
        dutch.as_deref(),
        Some(
            "Dit symbool is getekend op het verbindingsraster 4 mm, en deze pagina gebruikt QET 10 px. Plaatsen zet \
             het hier neer met zijn aansluitpunten op het raster van deze pagina."
        )
    );
    let key = shell.picker().unwrap().catalogue.symbols[i].key.clone();
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "place.pick", json!({"symbol": key}));
    done(&mut shell, "cursor.set", json!({"x_mm": 100, "y_mm": 100}));
    done(&mut shell, "canvas.activate", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    let tables = std::sync::Arc::clone(&shell.canvas().unwrap().page().tables);
    assert_eq!(tables.grid, (7_937_500, 3), "on the page in front");
    let grid = ew_api::symbols::units::Grid::QET_10PX;
    // A point of the QET grid is a third of a nanometre off a whole one: a placed point is the sum of two of them, so
    // within a nanometre of the grid.
    let on_grid = |v: i64| {
        let n = ew_api::symbols::units::Nm(v);
        grid.at(grid.snap(n)).is_some_and(|g| (g.0 - v).abs() <= 1)
    };
    assert_eq!(tables.cps.len(), 2, "the symbol is placed with its two points");
    for cp in &tables.cps {
        assert!(
            on_grid(cp.at.0) && on_grid(cp.at.1),
            "{} at {:?} is off the page's grid",
            cp.label,
            cp.at
        );
    }
    shell.shutdown();
}

/// On a QET-grid page placing a 4 mm drawing on the page in front is refused in the picker's words, both grids
/// named, and offers Place on a new page with its grid; the picker's greyed row of the saved symbol places it on a
/// new page with its grid, selected, and the picker closes.
#[test]
fn a_symbol_on_another_grid_is_placed_on_a_new_page_with_its_grid() {
    use ew_viewmodel::tools::PickerRow;
    let tmp = Tmp::new("symbol-new-page");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project.ew"),"template":"builtin"}),
    );
    let grid = |shell: &ShellModel| shell.canvas().unwrap().page().snapper.grid();
    drawn(&mut shell, "Offer block");
    assert_eq!(
        failed(&mut shell, "symbol.place", json!({"x":40,"y":40})),
        "This symbol is drawn on the 4 mm connection grid, and this page uses QET 10 px. Place it on a page with its \
         grid."
    );
    assert!(shell.symbol_editor().unwrap().offer_new_page);
    let pages = shell.page_labels().len();
    done(&mut shell, "symbol.scratch", json!({}));
    assert_eq!(grid(&shell), (4_000_000, 1));
    assert_eq!(shell.page_labels().len(), pages + 1);
    assert_eq!(shell.selection().items.len(), 1);
    // The picker: the saved symbol's greyed row, made current, is placed on a new page of its grid.
    done(&mut shell, "parts.new_symbol", json!({"like": "hand"}));
    assert!(!shell.symbol_editor().unwrap().offer_new_page);
    saved_and_left(&mut shell);
    done(&mut shell, "nav.goto", json!({"text": "1"}));
    assert_eq!(grid(&shell), (7_937_500, 3));
    done(&mut shell, "tool.place", json!({}));
    let picker = shell.picker().unwrap();
    let i = picker
        .catalogue
        .symbols
        .iter()
        .position(|s| s.key.starts_with("library:") && s.name.starts_with("Offer block"))
        .unwrap();
    let at = picker
        .rows
        .iter()
        .position(|r| *r == PickerRow::Symbol(i))
        .unwrap();
    assert_ne!(at, 0, "a starter symbol of the page's grid leads");
    assert_eq!(
        shell.dispatch("picker.new_page", &json!({})),
        Outcome::refused(ew_viewmodel::key!("reason-no-grid-mismatch")),
        "the first row is on this page's grid"
    );
    done(&mut shell, "picker.row", json!({"to": at}));
    let recorded = shell.recorded_commands().len();
    done(&mut shell, "picker.new_page", json!({}));
    assert!(shell.picker().is_none());
    assert_eq!(grid(&shell), (4_000_000, 1));
    assert_eq!(shell.page_labels().len(), pages + 2);
    let cmds: Vec<&str> = shell.recorded_commands()[recorded..]
        .iter()
        .map(|c| c.cmd.as_str())
        .collect();
    assert_eq!(cmds, ["page.create", "device.place"]);
    let selected = &shell.selection().items;
    assert_eq!(selected.len(), 1);
    assert_ne!(
        Some(selected[0].0.to_string()),
        shell.current_page().map(|p| p.uuid_text()),
        "the symbol is selected, not its page"
    );
    shell.shutdown();
}

/// A symbol file that cannot be read and a drawing that cannot be written are said in the window's words, never in
/// the reader's (a line and a column) or the operating system's.
#[test]
fn a_bad_symbol_file_and_a_failed_export_are_said_in_words() {
    let tmp = Tmp::new("symbol-file-words");
    let mut shell = tmp.empty_shell(Mode::Headless);
    let bad = tmp.0.join("bad.symbol.json");
    std::fs::write(&bad, "{ \"format\": ").unwrap();
    assert_eq!(
        failed(&mut shell, "symbol.open", json!({ "file": bad })),
        "This file cannot be read as a symbol. It may be damaged, or not a symbol file."
    );
    drawn(&mut shell, "Words");
    // A folder that is a file: the system refuses the write.
    let blocker = tmp.0.join("blocker");
    std::fs::write(&blocker, "x").unwrap();
    assert_eq!(
        failed(
            &mut shell,
            "symbol.export",
            json!({ "file": blocker.join("draft.symbol.json") })
        ),
        "The draft could not be written to that file. Check that the folder exists and that you may write to it."
    );
    shell.shutdown();
}

/// Place on a new page with its grid makes the page for the symbol alone: when the placement is then refused (a
/// terminal in a project without a terminal strip), the page is taken back, the page that was on show shows again,
/// and the refusal is said in words.
#[test]
fn a_refused_placement_on_a_new_page_leaves_no_empty_page() {
    let tmp = Tmp::new("symbol-new-page-refused");
    let dir = tmp.0.join("project.ew");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "Clamp");
    done(
        &mut shell,
        "parts.field",
        json!({"field":"symbol.what","value":"terminal"}),
    );
    done(&mut shell, "parts.save", json!({}));
    let labels =
        |shell: &ShellModel| -> Vec<String> { shell.page_labels().iter().map(ToString::to_string).collect() };
    let pages = labels(&shell);
    let folio = pages[shell.page_index()].clone();
    let refusal = failed(&mut shell, "symbol.new_page", json!({}));
    assert!(
        refusal.starts_with("No terminal strip to add to"),
        "{refusal}"
    );
    assert_eq!(labels(&shell), pages, "no page is left behind");
    assert_eq!(
        labels(&shell)[shell.page_index()],
        folio,
        "the page that was on show"
    );
    assert!(shell.symbol_editor().is_some_and(|e| e.error.is_some()));
    shell.shutdown();
}

/// Place on a new page places the saved symbol while the drawing is as saved and selects it; for any other drawing
/// it is refused in words.
#[test]
fn place_on_a_new_page_places_the_saved_symbol_only_while_the_drawing_is_as_saved() {
    let tmp = Tmp::new("symbol-offer-published");
    let dir = tmp.0.join("project.ew");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "Offered");
    let not_saved = "The symbol is not in a library as drawn. Save it to a library first.";
    assert_eq!(shell.symbol_editor().unwrap().published(), None);
    assert_eq!(failed(&mut shell, "symbol.new_page", json!({})), not_saved);
    done(&mut shell, "parts.save", json!({}));
    let key = shell.symbol_editor().unwrap().draft.key.clone();
    assert!(
        shell
            .symbol_editor()
            .unwrap()
            .published()
            .is_some_and(|a| a.starts_with("library:"))
    );
    // An edit makes it a drawing again; its undo makes it the saved one.
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":4,"x2":4,"y2":4}),
    );
    assert_eq!(shell.symbol_editor().unwrap().published(), None);
    done(&mut shell, "symbol.undo", json!({}));
    assert!(shell.symbol_editor().unwrap().published().is_some());
    let pages = shell.page_labels().len();
    let recorded = shell.recorded_commands().len();
    done(&mut shell, "symbol.new_page", json!({}));
    assert_eq!(shell.page_labels().len(), pages + 1);
    assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (4_000_000, 1));
    let cmds: Vec<&str> = shell.recorded_commands()[recorded..]
        .iter()
        .map(|c| c.cmd.as_str())
        .collect();
    assert_eq!(cmds, ["page.create", "device.place"]);
    assert_eq!(shell.selection().items.len(), 1);
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    // The project embeds the saved symbol under its own key, no test copy.
    assert_eq!(project_keys(&dir, &key), [key]);
}

/// A new free text is centred on its click, in the small style pin designations use until a style is chosen for a
/// text of the drawing, then in the style chosen last; a text may hold a line break; a style is named in words.
#[test]
fn a_new_text_is_centred_in_the_style_chosen_last() {
    use ew_viewmodel::shell::symbol_editor::text_style_words;
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-text-style");
    let mut shell = tmp.empty_shell(Mode::Headless);
    drawn(&mut shell, "Texts");
    let texts = |shell: &ShellModel| -> Vec<serde_json::Value> {
        let d = serde_json::to_value(&shell.symbol_editor().unwrap().draft).unwrap();
        d["symbol"]["graphics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|g| g.get("text").cloned())
            .collect()
    };
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"text","x":8,"y":8,"text":"K1"}),
    );
    let first = texts(&shell)[0].clone();
    assert_eq!(
        (&first["align"], &first["style"], &first["pos"]),
        (
            &json!("centre"),
            &json!("cp.small"),
            &json!([8_000_000, 8_000_000])
        )
    );
    // A style chosen for the text, which is selected as it is drawn, is the next text's.
    done(
        &mut shell,
        "symbol.prop",
        json!({"key": "style", "value": "tag.main"}),
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"text","x":8,"y":16,"text":"Line 1\nLine 2"}),
    );
    let second = texts(&shell)[1].clone();
    assert_eq!(
        (&second["align"], &second["style"], &second["text"]),
        (
            &json!("centre"),
            &json!("tag.main"),
            &json!({"en": "Line 1\nLine 2"})
        )
    );
    assert_eq!(text_style_words("cp.small"), "Pin designation");
    assert_eq!(text_style_words("company.label"), "company.label");
    assert!(strings::set_locale("nl"));
    let dutch = text_style_words("cp.small");
    assert!(strings::set_locale("en"));
    assert_eq!(dutch, "Aansluitcodering");
    shell.shutdown();
}

/// An edit the reader refuses (a text with no characters) is refused in words, never the parser's.
#[test]
fn an_edit_the_reader_refuses_is_refused_in_words() {
    let tmp = Tmp::new("symbol-edit-words");
    let mut shell = tmp.empty_shell(Mode::Headless);
    drawn(&mut shell, "Words");
    let before = shell.symbol_editor().unwrap().draft.clone();
    let sentence = "That change would leave the symbol incomplete, so it was not made. Fill in what it needs and try \
                    again.";
    assert_eq!(
        shell.dispatch("symbol.draw", &json!({"tool":"text","x":8,"y":8,"text":""})),
        Outcome::Failed {
            message: sentence.to_owned()
        }
    );
    let editor = shell.symbol_editor().unwrap();
    assert_eq!(
        (editor.error.as_deref(), &editor.draft),
        (Some(sentence), &before)
    );
    shell.shutdown();
}

/// The test copy's item IDs are v5 names whose parts are joined with U+001F (ADR-0004 amendment 4); a changed symbol
/// saved again takes the next patch version, and the toast names it.
#[test]
fn the_test_copys_ids_join_their_parts_with_the_unit_separator() {
    use ew_api::{output::Uuid, symbols};
    let tmp = Tmp::new("symbol-v5");
    let dir = tmp.0.join("project.ew");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "Five");
    done(&mut shell, "parts.save", json!({}));
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":4,"y":0,"x2":4,"y2":4}),
    );
    done(&mut shell, "parts.save", json!({}));
    assert_eq!(shell.symbol_editor().unwrap().draft.version, "1.0.1");
    assert!(!shell.symbol_editor().unwrap().dirty());
    assert_eq!(
        shell.toast().map(|t| t.text.clone()).as_deref(),
        Some("Saved Five as version 1.0.1 to the library My parts. Draw ▸ Symbol places it on a page.")
    );
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = ew_api::Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    let q = session.query();
    let (_, copy) = q
        .symbols()
        .iter()
        .find(|(_, s)| {
            s.item
                .meta
                .key
                .as_ref()
                .is_some_and(|k| k.as_str().starts_with("user.five.test."))
        })
        .unwrap();
    let key = copy.item.meta.key.as_ref().unwrap().to_string();
    assert_eq!(
        copy.item_id.as_uuid(),
        Uuid::new_v5(&symbols::DRAFT_LIBRARY, format!("symbol\u{1f}{key}").as_bytes())
    );
}

/// New symbol has a button on the Draw tab's Place group, and the command search names it there.
#[test]
fn new_symbol_has_a_button_on_the_draw_tab_and_the_search_groups_it_there() {
    use ew_viewmodel::command_bar::{Group, Tab};
    let tmp = Tmp::new("symbol-draw-tab");
    let mut shell = tmp.empty_shell(Mode::Gui);
    let place = ew_viewmodel::actions::find("parts.new_symbol")
        .unwrap()
        .placement
        .unwrap();
    assert_eq!((place.tab, place.group), (Tab::Draw, Group::Place));
    let bar = place.bar_label.unwrap();
    assert_eq!(ew_viewmodel::strings::text(bar), "New symbol");
    assert!(ew_viewmodel::strings::set_locale("nl"));
    let dutch = ew_viewmodel::strings::text(bar);
    assert!(ew_viewmodel::strings::set_locale("en"));
    assert_eq!(dutch, "Nieuw symbool");
    done(&mut shell, "tools.palette", json!({}));
    shell.set_palette_query("new symbol");
    let entry = shell
        .palette_entries()
        .into_iter()
        .find(|e| e.id == "parts.new_symbol")
        .unwrap();
    assert_eq!(entry.breadcrumb, "Draw · Place");
    shell.shutdown();
}

/// The quit question's Save symbol saves the drawing into My parts, then quits.
#[test]
fn the_quit_questions_save_symbol_saves_the_drawing_and_quits() {
    use ew_viewmodel::shell::ConfirmAnswer;
    let tmp = Tmp::new("symbol-quit-save");
    let mut shell = tmp.empty_shell(Mode::Gui);
    drawn(&mut shell, "Quit save");
    done(&mut shell, "app.quit", json!({}));
    assert!(shell.dialog().is_some() && !shell.should_quit());
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Save), Outcome::Done);
    assert!(shell.should_quit());
    assert!(shell.kept_symbol_editor().unwrap().stored());
    shell.shutdown();
}

/// The quit question's and the test page's titles name the symbol in the window's language.
#[test]
fn the_titles_name_the_symbol_in_the_windows_language() {
    use ew_viewmodel::dialogs::spec;
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-language");
    let dir = tmp.0.join("project.ew");
    let mut shell = tmp.empty_shell(Mode::Gui);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    assert!(strings::set_locale("nl"));
    drawn(&mut shell, "Voedingsblok");
    done(&mut shell, "app.quit", json!({}));
    let title = spec(shell.dialog().unwrap(), &shell).title;
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "symbol.scratch", json!({}));
    assert!(strings::set_locale("en"));
    assert_eq!(title, "Symbool Voedingsblok opslaan?");
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = ew_api::Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    let titles: Vec<String> = session
        .query()
        .pages()
        .iter()
        .map(|(_, p)| serde_json::to_value(&p.title).unwrap().to_string())
        .collect();
    assert!(
        titles.iter().any(|t| t.contains("Symbooltest: Voedingsblok")),
        "{titles:?}"
    );
}

/// A refused action names no member ("index", "idx", "start"), and an Image primitive has its word.
#[test]
fn refusals_of_the_drawings_actions_are_in_words() {
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-words");
    let mut shell = tmp.empty_shell(Mode::Headless);
    drawn(&mut shell, "Words");
    for (action, params, text) in [
        (
            "symbol.candidate",
            json!({"index": 5, "discard": true}),
            "Choose one of the drawings this file holds.",
        ),
        (
            "symbol.variant",
            json!({"variant": 9}),
            "Choose a variant from A to H.",
        ),
        (
            "symbol.cp",
            json!({"idx": 70000, "x": 0, "y": 0}),
            "Number a connection point from 1 to 65535.",
        ),
        (
            "symbol.draw",
            json!({"tool": "arc", "x": 0, "y": 0, "x2": 4, "y2": 4, "start": 400}),
            "Give the arc's start and sweep in whole degrees, from −360 to 360.",
        ),
    ] {
        assert_eq!(failed(&mut shell, action, params), text, "{action}");
    }
    assert_eq!(strings::text(ew_viewmodel::key!("symbol-image")), "Image");
    shell.shutdown();
}

/// The same content placed twice is one embedded copy, placed twice.
#[test]
fn the_same_content_placed_twice_reuses_one_copy() {
    let tmp = Tmp::new("symbol-twice");
    let dir = tmp.0.join("project.ew");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "Twice");
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "parts.new_symbol", json!({"like": "hand"}));
    done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = ew_api::Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    let q = session.query();
    let copies: Vec<_> = q
        .symbols()
        .iter()
        .filter(|(_, s)| {
            s.item
                .meta
                .key
                .as_ref()
                .is_some_and(|k| k.to_string().starts_with("user.twice.test."))
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(copies.len(), 1, "{copies:?}");
    let placed = q.reps().iter().filter(|(_, r)| r.symbol == copies[0]).count();
    assert_eq!(placed, 2);
}

/// In the window, New symbol over a drawing with changes asks Save symbol, Discard or Cancel in the house dialog, in
/// words that match its buttons; a drawing without changes is replaced at once.
#[test]
fn new_symbol_over_unsaved_changes_asks_save_discard_or_cancel() {
    use ew_viewmodel::dialogs::spec;
    use ew_viewmodel::shell::{ConfirmAnswer, Dialog};
    let tmp = Tmp::new("symbol-ask");
    let mut shell = tmp.empty_shell(Mode::Gui);
    let name = |shell: &ShellModel| {
        ew_viewmodel::shell::symbol_editor::display_name(&shell.symbol_editor().unwrap().draft)
    };
    drawn(&mut shell, "Ask block");
    done(&mut shell, "parts.new_symbol", json!({}));
    let dialog = shell.dialog().unwrap().clone();
    assert!(matches!(dialog, Dialog::ConfirmDiscard { symbol: true, .. }));
    let s = spec(&dialog, &shell);
    assert_eq!(s.title, "Save the symbol Ask block?");
    let labels: Vec<&str> = s.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Save symbol", "Cancel", "Discard changes"]);
    // Cancel keeps the drawing.
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Cancel), Outcome::Done);
    assert_eq!(name(&shell), "Ask block");
    // Discard starts the new one.
    done(&mut shell, "parts.new_symbol", json!({}));
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Discard), Outcome::Done);
    assert_eq!(name(&shell), "New symbol");
    // Save symbol saves the drawing first, then starts the new one.
    drawn(&mut shell, "Ask block");
    done(&mut shell, "parts.new_symbol", json!({}));
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Save), Outcome::Done);
    assert!(shell.dialog().is_none());
    assert_eq!(name(&shell), "New symbol");
    assert!(shell.settings().symbols_recent.first().is_some(), "saved");
    // A drawing without changes asks nothing.
    done(&mut shell, "parts.new_symbol", json!({}));
    assert!(shell.dialog().is_none());
    shell.shutdown();
}

/// A drawing with a rectangle, a line to a connection point, and the point: the things the selection tests act on.
/// The texts its kind needs (the device tag, the cross-reference of a contact) stand on it from the start.
fn with_things(tmp: &Tmp) -> ShellModel {
    let mut shell = tmp.empty_shell(Mode::Headless);
    new_drawing(&mut shell, "Select");
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"rectangle","x":4,"y":0,"x2":12,"y2":8}),
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":4,"x2":4,"y2":4}),
    );
    done(
        &mut shell,
        "symbol.cp",
        json!({"x":0,"y":4,"designation":"1","direction":"left"}),
    );
    shell
}

fn graphics(shell: &ShellModel) -> serde_json::Value {
    serde_json::to_value(&shell.symbol_editor().unwrap().draft.symbol.graphics).unwrap()
}

fn selection(shell: &ShellModel) -> Vec<String> {
    shell
        .symbol_editor()
        .unwrap()
        .selection
        .iter()
        .map(|i| i.word())
        .collect()
}

/// What a person learned on a page works on the drawing's selection: Select all, the arrows (a grid step; the
/// connection grid's with a connection point selected), Duplicate, Copy and Paste, Delete, each one undo step; a
/// click's Shift adds and Ctrl toggles.
#[test]
fn the_selection_is_moved_copied_and_deleted_as_on_a_page() {
    let tmp = Tmp::new("symbol-selection");
    let mut shell = with_things(&tmp);
    // A drawn thing is the one selected, so its rows show.
    assert_eq!(selection(&shell), ["cp:0"]);
    done(&mut shell, "symbol.select", json!({"items": "all"}));
    let all = selection(&shell);
    for thing in ["cp:0", "graphic:0", "graphic:1"] {
        assert!(all.iter().any(|s| s == thing), "{thing}: {all:?}");
    }
    // With a connection point selected an arrow moves everything one step of the connection grid.
    done(&mut shell, "symbol.nudge", json!({"dx": 1}));
    assert_eq!(
        graphics(&shell)[0]["rect"]["rect"],
        json!([[8_000_000, 0], [16_000_000, 8_000_000]])
    );
    let cp = shell.symbol_editor().unwrap().draft.symbol.cps.iter().next().unwrap().pos;
    assert_eq!((cp.x.0, cp.y.0), (4_000_000, 4_000_000));
    done(&mut shell, "symbol.undo", json!({}));
    assert_eq!(
        graphics(&shell)[0]["rect"]["rect"],
        json!([[4_000_000, 0], [12_000_000, 8_000_000]])
    );
    assert_eq!(selection(&shell), all, "the selection outlives an undo");
    // A shape alone moves by the drawing grid's step, which the person chooses.
    done(&mut shell, "symbol.select", json!({"items": ["graphic:0"]}));
    done(&mut shell, "symbol.nudge", json!({"dy": -1}));
    assert_eq!(
        graphics(&shell)[0]["rect"]["rect"][0],
        json!([4_000_000, -500_000])
    );
    done(&mut shell, "symbol.grid", json!({"step": 2}));
    done(&mut shell, "symbol.nudge", json!({"dy": 1}));
    assert_eq!(
        graphics(&shell)[0]["rect"]["rect"][0],
        json!([4_000_000, 1_500_000])
    );
    // Shift adds, Ctrl toggles.
    done(
        &mut shell,
        "symbol.select",
        json!({"items": ["graphic:1"], "mode": "add"}),
    );
    assert_eq!(selection(&shell), ["graphic:0", "graphic:1"]);
    done(
        &mut shell,
        "symbol.select",
        json!({"items": ["graphic:0"], "mode": "toggle"}),
    );
    assert_eq!(selection(&shell), ["graphic:1"]);
    // Duplicate: a copy one connection-grid step right and down, selected.
    done(&mut shell, "symbol.duplicate", json!({}));
    assert_eq!(graphics(&shell).as_array().unwrap().len(), 3);
    assert_eq!(selection(&shell), ["graphic:2"]);
    assert_eq!(graphics(&shell)[2]["line"]["from"], json!([4_000_000, 8_000_000]));
    // Copy, Paste and Cut; each paste lands a step further.
    done(&mut shell, "symbol.copy", json!({}));
    done(&mut shell, "symbol.paste", json!({}));
    done(&mut shell, "symbol.paste", json!({}));
    assert_eq!(
        graphics(&shell)[4]["line"]["from"],
        json!([12_000_000, 16_000_000])
    );
    done(&mut shell, "symbol.cut", json!({}));
    assert_eq!(graphics(&shell).as_array().unwrap().len(), 4);
    done(
        &mut shell,
        "symbol.select",
        json!({"items": ["graphic:2", "graphic:3"]}),
    );
    done(&mut shell, "symbol.delete", json!({}));
    assert_eq!(graphics(&shell).as_array().unwrap().len(), 2);
    assert!(selection(&shell).is_empty());
    // Nothing selected: the keys say so in words and change nothing.
    assert_eq!(
        failed(&mut shell, "symbol.delete", json!({})),
        "Nothing is selected. Click a thing in the drawing or in the list first."
    );
    shell.shutdown();
}

/// A deleted connection point takes the text that writes its number and its pin of the symbol's own function along,
/// so the symbol stays ready to save; a copied one gets a number and a pin of its own.
#[test]
fn a_connection_point_is_deleted_and_copied_with_its_pin() {
    use ew_api::symbols::schema::SlotKind;
    let tmp = Tmp::new("symbol-cp-delete");
    let mut shell = with_things(&tmp);
    let numbers = |shell: &ShellModel| {
        let d = &shell.symbol_editor().unwrap().draft;
        d.symbol
            .text_slots
            .iter()
            .filter(|s| matches!(s.kind, SlotKind::CpDesignation(_)))
            .count()
    };
    assert_eq!(numbers(&shell), 1);
    done(&mut shell, "symbol.select", json!({"items": ["cp:0"]}));
    done(&mut shell, "symbol.duplicate", json!({}));
    {
        let draft = &shell.symbol_editor().unwrap().draft;
        assert_eq!(draft.symbol.cps.len(), 2);
        assert_eq!(draft.function.cps.len(), 2, "the copy draws a pin of its own");
        assert!(draft.validate().is_empty(), "{:?}", draft.validate());
    }
    done(&mut shell, "symbol.select", json!({"items": ["cp:0"]}));
    done(&mut shell, "symbol.delete", json!({}));
    let draft = &shell.symbol_editor().unwrap().draft;
    assert_eq!((draft.symbol.cps.len(), draft.function.cps.len()), (1, 1));
    assert_eq!(numbers(&shell), 0, "the number's text went with its point");
    assert!(draft.validate().is_empty(), "{:?}", draft.validate());
    shell.shutdown();
}

/// The symbol's short sheet holds Name and What it is; Properties follows the selection, and with nothing selected
/// says so. What it is, chosen in a panel builder's words, sets what the program does with the symbol; a row set with
/// several things selected changes each; a shape's size and a handle reshape it; Line up puts the selected things on
/// one edge.
#[test]
fn the_rows_of_properties_follow_the_selection_and_set_every_selected_thing() {
    use ew_viewmodel::shell::symbol_editor::RowValue;
    let tmp = Tmp::new("symbol-sheet");
    let mut shell = with_things(&tmp);
    done(
        &mut shell,
        "symbol.cp",
        json!({"x":0,"y":8,"designation":"2","direction":"left"}),
    );
    done(&mut shell, "symbol.select", json!({"items": []}));
    let sheet = shell.symbol_sheet().unwrap();
    assert!(sheet.rows.is_empty(), "nothing selected: {:?}", sheet.rows);
    assert_eq!(
        sheet.line,
        "Nothing is selected. Click a thing in the drawing or in the list first."
    );
    let short = shell.symbol_editor().unwrap().symbol_rows();
    let keys: Vec<&str> = short.iter().map(|r| r.key).collect();
    assert_eq!(keys, ["name", "what"]);
    let what = short.iter().find(|r| r.key == "what").unwrap();
    assert!(
        matches!(&what.value, RowValue::Choice { value: Some(v), .. } if v == "contact-no"),
        "a first symbol is a normally open contact: {what:?}"
    );
    // A pressure switch is one of the kinds a panel builder draws: a contact, listed with the sensors.
    done(
        &mut shell,
        "parts.field",
        json!({"field": "symbol.what", "value": "pressure-switch"}),
    );
    let function = serde_json::to_value(&shell.symbol_editor().unwrap().draft.function).unwrap();
    assert_eq!(function["contact_kind"], "no");
    assert_eq!(function["category"][0], "SensingPressure");
    assert_eq!(
        shell.symbol_editor().unwrap().outline().what,
        "Pressure switch or sensor \u{b7} Electrical"
    );
    // Two connection points: the rows they share; a value that differs is not shown as either's.
    done(&mut shell, "symbol.select", json!({"items": ["cp:0", "cp:1"]}));
    let sheet = shell.symbol_sheet().unwrap();
    assert_eq!(sheet.title, "2 things selected");
    let y = sheet.rows.iter().find(|r| r.key == "y").unwrap();
    assert_eq!(y.value, RowValue::Mm(None), "4 and 8 mm");
    done(&mut shell, "symbol.prop", json!({"key": "dir", "value": "up"}));
    done(
        &mut shell,
        "symbol.prop",
        json!({"key": "role", "value": "l_plus"}),
    );
    let draft = &shell.symbol_editor().unwrap().draft;
    let dirs: Vec<_> = draft
        .symbol
        .cps
        .iter()
        .map(|c| serde_json::to_value(&c.dir).unwrap())
        .collect();
    assert_eq!(dirs, [json!("up"), json!("up")]);
    let roles: Vec<_> = draft
        .function
        .cps
        .iter()
        .map(|c| serde_json::to_value(&c.potential_role).unwrap())
        .collect();
    assert_eq!(roles, [json!("l_plus"), json!("l_plus")]);
    // The outline names each thing: a connection point by its designation and what it carries.
    let outline = shell.symbol_editor().unwrap().outline();
    let points = &outline.groups[0];
    assert_eq!(
        points
            .rows
            .iter()
            .map(|r| (r.label.as_str(), r.detail.as_str()))
            .collect::<Vec<_>>(),
        [("1", "L+"), ("2", "L+")]
    );
    assert!(outline.todo.is_empty(), "ready to save: {:?}", outline.todo);
    // A rectangle's rows are its place and size; a handle drags a corner.
    done(&mut shell, "symbol.select", json!({"items": ["graphic:0"]}));
    done(&mut shell, "symbol.prop", json!({"key": "w", "value": 10}));
    done(&mut shell, "symbol.prop", json!({"key": "x", "value": 6}));
    assert_eq!(
        graphics(&shell)[0]["rect"]["rect"],
        json!([[6_000_000, 0], [16_000_000, 8_000_000]])
    );
    done(&mut shell, "symbol.grip", json!({"grip": 2, "x": 20, "y": 12}));
    assert_eq!(
        graphics(&shell)[0]["rect"]["rect"],
        json!([[6_000_000, 0], [20_000_000, 12_000_000]])
    );
    // A value a row does not take is refused in words.
    assert_eq!(
        failed(&mut shell, "symbol.prop", json!({"key": "w", "value": -1})),
        "That value does not fit this row, so nothing was changed."
    );
    // Line up: the line's left end comes to the rectangle's left edge.
    done(
        &mut shell,
        "symbol.select",
        json!({"items": ["graphic:0", "graphic:1"]}),
    );
    done(&mut shell, "symbol.align", json!({"to": "right"}));
    assert_eq!(graphics(&shell)[1]["line"]["to"], json!([20_000_000, 4_000_000]));
    // An issue is one thing to do, with the thing it is about.
    done(&mut shell, "symbol.select", json!({"items": ["cp:0"]}));
    done(&mut shell, "symbol.prop", json!({"key": "x", "value": 0.5}));
    let todo = shell.symbol_editor().unwrap().outline().todo;
    assert_eq!(todo.len(), 1);
    assert_eq!(todo[0].item.map(|i| i.word()), Some("cp:0".to_owned()));
    shell.shutdown();
}

/// A connection point put on the end of a lead faces the way the lead points out of the symbol (Along its lead, the
/// tool's first direction), so a wire carries straight on from it; one where no lead ends keeps the default.
#[test]
fn a_connection_point_on_a_lead_faces_along_it() {
    let tmp = Tmp::new("symbol-cp-lead");
    let mut shell = tmp.empty_shell(Mode::Headless);
    new_drawing(&mut shell, "Pressure switch");
    for (y, y2) in [(-4, -12), (4, 12)] {
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool": "line", "x": 0, "y": y, "x2": 0, "y2": y2}),
        );
    }
    for (y, designation) in [(-12, "1"), (12, "2")] {
        done(
            &mut shell,
            "symbol.cp",
            json!({"x": 0, "y": y, "designation": designation, "direction": "lead"}),
        );
    }
    done(
        &mut shell,
        "symbol.cp",
        json!({"x": 20, "y": 0, "designation": "3", "direction": "lead"}),
    );
    let dirs: Vec<String> = shell
        .symbol_editor()
        .unwrap()
        .draft
        .symbol
        .cps
        .iter()
        .map(|c| serde_json::to_value(&c.dir).unwrap().as_str().unwrap().to_owned())
        .collect();
    assert_eq!(dirs, ["up", "down", "left"]);
}

/// While the drawing shows, the command search offers what the dispatch does: a page's tool greyed with the
/// drawing's reason, as the menus grey it, and Save, which saves the symbol, enabled.
#[test]
fn the_command_search_on_the_drawing_greys_what_the_drawing_refuses() {
    let tmp = Tmp::new("symbol-palette");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project"),"template":"builtin"}),
    );
    drawn(&mut shell, "Searched contact");
    done(&mut shell, "tools.palette", json!({}));
    for (query, id, enabled) in [("wire", "tool.wire", false), ("save", "file.save", true)] {
        shell.set_palette_query(query);
        let entry = shell
            .palette_entries()
            .into_iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("{id} is offered for {query:?}"));
        assert_eq!(entry.enabled, enabled, "{id}: {:?}", entry.reason);
        let refused = matches!(shell.dispatch(id, &json!({})), Outcome::Refused { .. });
        assert_eq!(refused, !enabled, "{id} runs as the search offers it");
        if !enabled {
            assert_eq!(
                entry.reason.as_deref(),
                Some(ew_viewmodel::strings::text(ew_viewmodel::key!(
                    "symbol-editor-active"
                )))
            );
        }
        if shell.palette().is_none() {
            done(&mut shell, "tools.palette", json!({}));
        }
    }
}

/// The describe text prints the command bar the window shows: over Parts with a drawing its tab Drawing, never
/// collapsed, though the project's bar is collapsed under it.
#[test]
fn the_describe_text_prints_the_bar_the_window_shows_over_the_drawing() {
    let tmp = Tmp::new("symbol-describe-bar");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project"),"template":"builtin"}),
    );
    done(&mut shell, "view.command_bar", json!({}));
    new_drawing(&mut shell, "Described");
    let text = shell.describe();
    let tab = text
        .lines()
        .find(|l| l.starts_with("commandbar tab "))
        .expect("the command bar's tab line");
    assert_eq!(tab, "commandbar tab \"Drawing\"");
    shell.shutdown();
}
