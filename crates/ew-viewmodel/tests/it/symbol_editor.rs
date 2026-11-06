// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

#[test]
fn published_symbol_is_available_in_insert_and_scratch_retains_its_own_identity() {
    let tmp = Tmp::new("symbol-publish-insert");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project"),"template":"builtin"}),
    );
    done(
        &mut shell,
        "command.run",
        json!({"cmd":"page.create","params":{"number":2,"grid":{"num":4000000,"den":1}}}),
    );
    done(&mut shell, "page.goto", json!({"folio":"2"}));
    done(&mut shell, "tool.place", json!({})); // Prime the catalogue before publication.
    done(&mut shell, "ui.dismiss", json!({}));
    done(
        &mut shell,
        "symbol.new",
        json!({"key":"example.published","name":"Published contact"}),
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"rectangle","x":0,"y":0,"x2":8,"y2":8}),
    );
    done(&mut shell, "symbol.cp", json!({"x":0,"y":0,"designation":"1"}));
    done(
        &mut shell,
        "symbol.save",
        json!({"directory":tmp.0.join("symbols.ewlib")}),
    );
    done(&mut shell, "symbol.close", json!({}));
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
    assert!(
        !row.black_box,
        "authored geometry must use ordinary symbol placement"
    );
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "place.pick", json!({"symbol":row.key,"letter":"U"}));
    done(&mut shell, "cursor.set", json!({"x_mm":40,"y_mm":40}));
    done(&mut shell, "canvas.activate", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "tool.place", json!({}));
    assert!(
        !shell
            .picker()
            .unwrap()
            .catalogue
            .symbols
            .iter()
            .find(|r| r.key == "example.published")
            .unwrap()
            .black_box
    );
    done(&mut shell, "ui.dismiss", json!({}));
    done(
        &mut shell,
        "symbol.fork",
        json!({"source":row.key,"key":"example.scratch"}),
    );
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "symbol.close", json!({"discard":true}));
    done(&mut shell, "file.save", json!({}));
    let path = tmp.0.join("project");
    shell.shutdown();
    let (session, _) = ew_viewmodel::ew_api::Session::open_read_only(&path, "ElektraWatt test").unwrap();
    let q = session.query();
    let scratch = q
        .symbols()
        .iter()
        .find(|(_, s)| {
            s.item
                .meta
                .key
                .as_ref()
                .is_some_and(|k| k.as_str().starts_with("example.scratch.test."))
        })
        .unwrap()
        .1
        .clone();
    assert_eq!(
        scratch.library_id.as_uuid(),
        ew_viewmodel::ew_api::symbols::DRAFT_LIBRARY
    );
    assert!(q.symbols().iter().any(|(_, s)| {
        s.item
            .meta
            .key
            .as_ref()
            .is_some_and(|k| k.as_str() == "example.published")
    }));
    assert!(
        session
            .place_catalogue("2")
            .unwrap()
            .symbols
            .iter()
            .filter(|s| s.key == "example.published")
            .all(|s| !s.black_box)
    );
    drop(session);
    // A fresh shell has no in-memory tiers from the publication operation.
    let mut reopened = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut reopened, "symbol.new", json!({"key":"example.next"}));
    assert!(
        reopened
            .symbol_editor()
            .unwrap()
            .sources
            .iter()
            .any(|s| s.key.as_deref() == Some("example.published") && s.library.is_some())
    );
    reopened.shutdown();
}

#[test]
fn pin_mapping_swaps_atomically_without_changing_function_data() {
    for mode in [Mode::Headless, Mode::Gui] {
        let tmp = Tmp::new("symbol-map");
        let mut shell = ShellModel::new(mode, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
        done(
            &mut shell,
            "symbol.new",
            json!({"key":"example.contact","name":"Contact"}),
        );
        done(&mut shell, "symbol.function", json!({"key":"ew.fn.contact-no"}));
        done(
            &mut shell,
            "symbol.cp",
            json!({"idx":1,"fn_cp":0,"x":0,"y":0,"direction":"up"}),
        );
        done(
            &mut shell,
            "symbol.cp",
            json!({"idx":2,"fn_cp":1,"x":0,"y":8,"direction":"down"}),
        );
        let before = shell.symbol_editor().unwrap().draft.clone();
        done(&mut shell, "symbol.map_cp", json!({"cp":1,"fn_cp":1}));
        let after = shell.symbol_editor().unwrap().draft.clone();
        assert_eq!(after.symbol.cps.get(&1).unwrap().fn_cp, 1);
        assert_eq!(after.symbol.cps.get(&2).unwrap().fn_cp, 0);
        assert_eq!(after.function, before.function);
        assert_eq!(after.function_ref, before.function_ref);
        assert_eq!(
            after.symbol.cps.get(&1).unwrap().pos,
            before.symbol.cps.get(&1).unwrap().pos
        );
        done(&mut shell, "symbol.undo", json!({}));
        assert_eq!(shell.symbol_editor().unwrap().draft, before);
        done(&mut shell, "symbol.redo", json!({}));
        assert_eq!(shell.symbol_editor().unwrap().draft, after);
        assert_eq!(shell.dispatch("symbol.map_cp", &json!({"cp":1,"fn_cp":65000})), Outcome::Failed {
            message: "Symbol editor: Choose a physical function pin with the same discipline. A used pin can only swap with another compatible pin.".into()
        });
        assert_eq!(shell.symbol_editor().unwrap().draft, after);
        done(&mut shell, "symbol.undo", json!({}));
        assert_eq!(shell.symbol_editor().unwrap().error, None);
        shell.shutdown();
    }
}

#[test]
fn authored_qet_symbol_without_key_suffix_keeps_grid_and_shape_after_insertion() {
    let tmp = Tmp::new("symbol-qet-catalogue");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project"),"template":"builtin"}),
    );
    done(
        &mut shell,
        "command.run",
        json!({"cmd":"page.create","params":{"number":2,"grid":{"num":7937500,"den":3}}}),
    );
    done(
        &mut shell,
        "command.run",
        json!({"cmd":"page.create","params":{"number":3,"grid":{"num":4000000,"den":1}}}),
    );
    done(&mut shell, "page.goto", json!({"folio":"2"}));
    done(
        &mut shell,
        "symbol.new",
        json!({"key":"example.import","name":"Imported graphic"}),
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":4,"y2":0}),
    );
    let mut doc = serde_json::to_value(&shell.symbol_editor().unwrap().draft).unwrap();
    doc["symbol"]["grid"] = json!({"num":7937500,"den":3});
    done(&mut shell, "symbol.replace", json!({"document":doc}));
    done(
        &mut shell,
        "symbol.save",
        json!({"directory":tmp.0.join("imported.ewlib")}),
    );
    done(&mut shell, "symbol.close", json!({}));
    done(&mut shell, "tool.place", json!({}));
    let key = shell
        .picker()
        .unwrap()
        .catalogue
        .symbols
        .iter()
        .find(|r| r.name.starts_with("Imported graphic"))
        .unwrap()
        .key
        .clone();
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "place.pick", json!({"symbol":key,"letter":"U"}));
    assert_eq!(shell.tool().name(), "place");
    done(&mut shell, "cursor.set", json!({"x_mm":40,"y_mm":40}));
    done(&mut shell, "canvas.activate", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "tool.place", json!({}));
    let row = shell
        .picker()
        .unwrap()
        .catalogue
        .symbols
        .iter()
        .find(|r| r.key == "example.import")
        .unwrap();
    assert!(!row.black_box);
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) =
        ew_viewmodel::ew_api::Session::open_read_only(&tmp.0.join("project"), "ElektraWatt test").unwrap();
    assert!(
        session
            .place_catalogue("2")
            .unwrap()
            .symbols
            .iter()
            .any(|r| r.key == "example.import" && !r.black_box)
    );
    assert!(
        !session
            .place_catalogue("3")
            .unwrap()
            .symbols
            .iter()
            .any(|r| r.key == "example.import")
    );
}

#[test]
fn standalone_library_chooser_and_fork_keep_the_source_independent() {
    let tmp = Tmp::new("symbol-fork");
    let mut shell = ShellModel::new(
        Mode::Gui,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "symbol.new",
        json!({"key":"example.empty","name":"Empty"}),
    );
    let editor = shell.symbol_editor().unwrap();
    assert!(editor.functions.iter().any(|f| f.key == "ew.fn.contact-no"));
    let source = editor
        .sources
        .iter()
        .find(|s| s.key.as_deref() == Some("ew.iec.contact-no"))
        .unwrap()
        .source
        .clone();
    done(
        &mut shell,
        "symbol.fork",
        json!({"source":source,"key":"example.fork","discard":true}),
    );
    let fork = shell.symbol_editor().unwrap().draft.clone();
    assert_eq!(fork.key, "example.fork");
    assert!(fork.function_ref.is_none());
    assert!(!fork.symbol.graphics.is_empty());
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":4,"y2":0}),
    );
    done(
        &mut shell,
        "symbol.fork",
        json!({"source":source,"key":"example.fork","discard":true}),
    );
    assert_eq!(shell.symbol_editor().unwrap().draft, fork);
    shell.shutdown();
}

#[test]
fn project_and_library_sources_with_the_same_key_fork_the_selected_copy() {
    use ew_viewmodel::ew_api::symbols;
    let tmp = Tmp::new("symbol-source-identity");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project"),"template":"builtin"}),
    );
    done(
        &mut shell,
        "symbol.fork",
        json!({"source":"ew.iec.contact-no","key":"ew.iec.contact-no"}),
    );
    let original = shell.symbol_editor().unwrap().draft.clone();
    let installed = shell
        .symbol_editor()
        .unwrap()
        .sources
        .iter()
        .find(|s| s.key.as_deref() == Some("ew.iec.contact-no"))
        .unwrap()
        .source
        .clone();
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":-4,"y":0,"x2":4,"y2":0}),
    );
    let changed = shell.symbol_editor().unwrap().draft.clone();
    let copies = changed.documents(symbols::DRAFT_LIBRARY).unwrap();
    let embedded = copies
        .iter()
        .find_map(|d| match d {
            symbols::schema::LibDocument::Symbol(s) => Some(s.id.to_string()),
            _ => None,
        })
        .unwrap();
    done(&mut shell, "symbol.close", json!({"discard":true}));
    done(
        &mut shell,
        "command.run",
        json!({"cmd":"library.embed","params":{"copies":copies}}),
    );
    for (source, graphics) in [
        (&embedded, &changed.symbol.graphics),
        (&installed, &original.symbol.graphics),
    ] {
        done(
            &mut shell,
            "symbol.fork",
            json!({"source":source,"key":"example.copy","discard":true}),
        );
        assert_eq!(&shell.symbol_editor().unwrap().draft.symbol.graphics, graphics);
    }
    shell.shutdown();
}

#[test]
fn equal_function_keys_in_two_libraries_retain_their_own_pin_data() {
    let tmp = Tmp::new("symbol-function-identity");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let mut first = String::new();
    for designation in ["A", "B"] {
        done(
            &mut shell,
            "symbol.new",
            json!({"key":"example.shared","name":designation,"discard":true}),
        );
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4}),
        );
        done(
            &mut shell,
            "symbol.cp",
            json!({"x":0,"y":0,"designation":designation}),
        );
        done(
            &mut shell,
            "symbol.save",
            json!({"directory":tmp.0.join(format!("{designation}.ewlib"))}),
        );
        done(
            &mut shell,
            "symbol.new",
            json!({"key":"example.empty","name":"Empty","discard":true}),
        );
        if designation == "A" {
            first = shell
                .symbol_editor()
                .unwrap()
                .functions
                .iter()
                .find(|f| f.key == "example.shared.function")
                .unwrap()
                .source
                .clone();
        }
    }
    let choices: Vec<_> = shell
        .symbol_editor()
        .unwrap()
        .functions
        .iter()
        .filter(|f| f.key == "example.shared.function")
        .map(|f| f.source.clone())
        .collect();
    assert_eq!(choices.len(), 2);
    for source in choices {
        done(&mut shell, "symbol.function", json!({"key":source}));
        assert_eq!(
            shell
                .symbol_editor()
                .unwrap()
                .draft
                .function
                .cps
                .get(&1)
                .unwrap()
                .designation
                .as_str(),
            if source == first { "A" } else { "B" }
        );
    }
    shell.shutdown();
}

#[test]
fn editor_f1_opens_its_reference_in_both_frontends() {
    for mode in [Mode::Headless, Mode::Gui] {
        let tmp = Tmp::new("symbol-help");
        let mut shell = ShellModel::new(mode, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
        done(
            &mut shell,
            "symbol.new",
            json!({"key":"example.help","name":"Help"}),
        );
        done(&mut shell, "help.documentation", json!({}));
        assert_eq!(
            shell.last_help(),
            Some("reference/symbol-editor.html#symbol-editor-and-automated-authoring")
        );
        assert_eq!(
            ew_viewmodel::help::action_topic("symbol.new"),
            Some("symbol-editor")
        );
        done(&mut shell, "symbol.close", json!({"discard":true}));
        done(&mut shell, "help.documentation", json!({}));
        assert_eq!(
            shell.last_help(),
            Some("how-to/application-window.html#start-page")
        );
        shell.shutdown();
    }
}

#[test]
fn editor_power_block_history_export_save_and_place_in_both_frontends() {
    for mode in [Mode::Headless, Mode::Gui] {
        let tmp = Tmp::new("symbol-editor");
        let mut shell = ShellModel::new(mode, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
        done(
            &mut shell,
            "file.new",
            json!({"dir":tmp.0.join("scratch.ew"),"template":"builtin"}),
        );
        done(&mut shell, "page.goto", json!({"folio":"1"}));
        done(
            &mut shell,
            "symbol.new",
            json!({"key":"example.power","name":"Power block"}),
        );
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
        done(&mut shell, "symbol.slot", json!({"x":0,"y":-4,"kind":"tag"}));
        done(&mut shell, "symbol.validate", json!({}));
        let original = shell.symbol_editor().unwrap().draft.clone();
        done(&mut shell, "symbol.cp", json!({"idx":1,"x":4.5,"y":4}));
        assert_eq!(shell.dispatch("symbol.validate",&json!({})),Outcome::Failed{message:"Symbol editor: Connection point 1 is off the connection grid. Move it onto the grid before saving.".into()});
        done(&mut shell, "edit.undo", json!({}));
        assert_eq!(shell.symbol_editor().unwrap().draft, original);
        done(&mut shell, "edit.redo", json!({}));
        assert_eq!(
            shell
                .symbol_editor()
                .unwrap()
                .draft
                .symbol
                .cps
                .iter()
                .next()
                .unwrap()
                .pos
                .x
                .0,
            4_500_000
        );
        done(&mut shell, "edit.undo", json!({}));
        let file = tmp.0.join("power.symbol.json");
        done(&mut shell, "symbol.export", json!({"file":file}));
        done(
            &mut shell,
            "symbol.save",
            json!({"directory":tmp.0.join("symbols.ewlib")}),
        );
        done(&mut shell, "symbol.variant", json!({"variant":7}));
        let pages = shell.page_count();
        assert!(matches!(
            shell.dispatch("symbol.place", &json!({"x":80,"y":80})),
            Outcome::Failed { .. }
        ));
        assert_eq!(shell.page_count(), pages);
        done(&mut shell, "symbol.scratch", json!({}));
        assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (4_000_000, 1));
        // ID-1463: the page shows after a test placement; the editor keeps the draft, Edit ▸ Symbol editor returns.
        assert!(shell.symbol_editor().is_none());
        done(&mut shell, "symbol.new", json!({}));
        done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
        done(&mut shell, "symbol.new", json!({}));
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool":"line","x":1,"y":1,"x2":2,"y2":2}),
        );
        done(&mut shell, "symbol.place", json!({"x":120,"y":80}));
        done(&mut shell, "symbol.new", json!({}));
        let draft = shell.symbol_editor().unwrap().draft.clone();
        let mut qet = serde_json::to_value(&draft).unwrap();
        let grid = ew_viewmodel::ew_api::symbols::units::Grid::QET_10PX;
        qet["symbol"]["grid"] = serde_json::to_value(grid).unwrap();
        for (i, pin) in qet["symbol"]["cps"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            pin["pos"] = json!([0, grid.at(i as i64 + 1).unwrap().0]);
        }
        done(&mut shell, "symbol.replace", json!({"document":qet}));
        done(&mut shell, "symbol.validate", json!({}));
        let page_count = shell.page_count();
        assert!(
            matches!(shell.dispatch("symbol.place", &json!({"x":80,"y":80})), Outcome::Failed { message } if message.contains("different connection grid"))
        );
        assert_eq!(shell.page_count(), page_count);
        done(&mut shell, "symbol.replace", json!({"document":draft}));
        // Close keeps the unsaved draft; another symbol is refused until it is discarded.
        done(&mut shell, "symbol.close", json!({}));
        assert!(shell.symbol_editor().is_none());
        assert_eq!(shell.kept_symbol_editor().unwrap().draft, draft);
        assert_eq!(
            shell.dispatch("symbol.open", &json!({"file":file})),
            Outcome::Failed {
                message: "Symbol editor: This symbol has unsaved changes. Save it or choose Discard changes."
                    .into()
            }
        );
        done(&mut shell, "symbol.close", json!({"discard":true}));
        assert!(shell.kept_symbol_editor().is_none());
        done(&mut shell, "symbol.open", json!({"file":file}));
        assert_eq!(shell.symbol_editor().unwrap().draft, original);
        shell.shutdown();
    }
}

#[test]
fn moving_an_assigned_pin_keeps_its_mapping_and_scratch_uses_a_free_folio() {
    for mode in [Mode::Headless, Mode::Gui] {
        let tmp = Tmp::new("symbol-scratch");
        let mut shell = ShellModel::new(mode, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
        done(
            &mut shell,
            "file.new",
            json!({"dir":tmp.0.join("scratch.ew"),"template":"builtin"}),
        );
        done(
            &mut shell,
            "symbol.new",
            json!({"key":"example.contact","name":"Contact"}),
        );
        done(&mut shell, "symbol.function", json!({"key":"ew.fn.contact-no"}));
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool":"line","x":0,"y":0,"x2":0,"y2":8}),
        );
        done(
            &mut shell,
            "symbol.cp",
            json!({"idx":1,"fn_cp":0,"x":0,"y":0,"direction":"up"}),
        );
        done(
            &mut shell,
            "symbol.cp",
            json!({"idx":2,"fn_cp":1,"x":0,"y":8,"direction":"down"}),
        );
        let before = shell
            .symbol_editor()
            .unwrap()
            .draft
            .symbol
            .cps
            .iter()
            .next()
            .unwrap()
            .clone();
        done(&mut shell, "symbol.cp", json!({"idx":1,"x":4,"y":0}));
        let after = shell
            .symbol_editor()
            .unwrap()
            .draft
            .symbol
            .cps
            .iter()
            .next()
            .unwrap();
        assert_eq!(after.fn_cp, before.fn_cp);
        assert_eq!(after.dir, before.dir);
        done(&mut shell, "symbol.validate", json!({}));
        let pages = shell.page_count();
        done(&mut shell, "symbol.scratch", json!({}));
        assert_eq!(shell.page_count(), pages + 1);
        done(&mut shell, "symbol.scratch", json!({}));
        assert_eq!(shell.page_count(), pages + 2);
        shell.shutdown();
    }
}

#[test]
fn project_save_confirmation_saves_the_project_with_a_symbol_open() {
    use ew_viewmodel::shell::ConfirmAnswer;
    for pending in ["file.close", "app.quit"] {
        let tmp = Tmp::new("symbol-project-confirm");
        let mut shell = ShellModel::new(
            Mode::Gui,
            ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
        );
        done(
            &mut shell,
            "file.new",
            json!({"dir":tmp.0.join("project.ew"),"template":"builtin"}),
        );
        done(&mut shell, "page.new", json!({"folio":"42"}));
        done(
            &mut shell,
            "symbol.new",
            json!({"key":"example.test","name":"Test"}),
        );
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

/// A one-pin drawing keyed `key` in a fresh editor.
fn drawn(shell: &mut ShellModel, key: &str, name: &str) {
    done(shell, "symbol.new", json!({"key":key,"name":name,"discard":true}));
    done(
        shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4}),
    );
    done(shell, "symbol.cp", json!({"x":0,"y":0,"designation":"1"}));
}

fn failed(shell: &mut ShellModel, action: &str, params: serde_json::Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action}: {other:?}"),
    }
}

#[test]
fn a_second_symbol_saved_on_the_same_key_is_refused_and_the_first_stays() {
    let tmp = Tmp::new("symbol-same-key");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let library = json!({"directory":tmp.0.join("symbols.ewlib")});
    drawn(&mut shell, "example.shared", "First");
    done(&mut shell, "symbol.save", library.clone());
    let editor = shell.symbol_editor().unwrap();
    assert!(editor.draft.identity.is_some() && !editor.dirty());
    drawn(&mut shell, "example.shared", "Second");
    let refusal = "Symbol editor: The library already holds a different symbol with the key example.shared. Choose \
                   another key.";
    assert_eq!(failed(&mut shell, "symbol.save", library.clone()), refusal);
    // The version bump that the immutability refusal asks for does not change that.
    let mut doc = serde_json::to_value(&shell.symbol_editor().unwrap().draft).unwrap();
    doc["version"] = json!("1.0.1");
    done(&mut shell, "symbol.replace", json!({"document":doc}));
    assert_eq!(failed(&mut shell, "symbol.save", library), refusal);
    assert!(shell.symbol_editor().unwrap().draft.identity.is_none());
    done(
        &mut shell,
        "symbol.new",
        json!({"key":"example.next","discard":true}),
    );
    let names: Vec<_> = shell
        .symbol_editor()
        .unwrap()
        .sources
        .iter()
        .filter(|s| s.key.as_deref() == Some("example.shared"))
        .map(|s| serde_json::to_value(&s.name).unwrap())
        .collect();
    assert_eq!(names, [json!({"en":"First"})]);
    shell.shutdown();
}

#[test]
fn a_new_symbol_has_no_shared_default_key() {
    let tmp = Tmp::new("symbol-no-default-key");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut shell, "symbol.new", json!({}));
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4}),
    );
    assert_eq!(shell.symbol_editor().unwrap().draft.key, "");
    assert_eq!(
        failed(
            &mut shell,
            "symbol.save",
            json!({"directory":tmp.0.join("symbols.ewlib")})
        ),
        "Symbol editor: Give the symbol a key."
    );
    assert_eq!(
        failed(
            &mut shell,
            "symbol.fork",
            json!({"source":"ew.iec.contact-no","key":"","discard":true})
        ),
        "Symbol editor: Give the symbol a key."
    );
    // An import without a key takes its file's name.
    let file = tmp.0.join("contact no.elmt");
    std::fs::write(
        &file,
        include_bytes!("../../../ew-import-qet/tests/fixtures/contact_no.elmt"),
    )
    .unwrap();
    done(&mut shell, "symbol.open", json!({"file":file,"discard":true}));
    assert_eq!(shell.symbol_editor().unwrap().draft.key, "contact_no");
    shell.shutdown();
}

#[test]
fn an_assigned_function_is_required_as_compatible() {
    let tmp = Tmp::new("symbol-function-caret");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "symbol.new",
        json!({"key":"example.contact","name":"Contact"}),
    );
    done(&mut shell, "symbol.function", json!({"key":"ew.fn.contact-no"}));
    let draft = &shell.symbol_editor().unwrap().draft;
    let reference = draft.function_ref.as_ref().unwrap();
    assert!(reference.version_req.starts_with('^'), "{reference:?}");
    shell.shutdown();
}

#[test]
fn saving_without_configured_tiers_keeps_the_index_in_the_settings_folder() {
    let tmp = Tmp::new("symbol-tiers");
    let config = tmp.0.join("config");
    let mut shell = ShellModel::new(Mode::Headless, ShellConfig::in_dir(&config, "0.0.0-test"));
    drawn(&mut shell, "example.tiers", "Tiers");
    done(
        &mut shell,
        "symbol.save",
        json!({"directory":tmp.0.join("out").join("symbols.ewlib")}),
    );
    assert!(!tmp.0.join("out").join(".symbol-index").exists());
    assert!(config.join("library-index").is_dir());
    shell.shutdown();
    // A library of the user tier opens from it: it is not remembered as a company library.
    let data = tmp.0.join("data");
    let tiers =
        ew_viewmodel::ew_api::library::LibraryTiers::under(&data, &tmp.0.join("cache"), vec![], vec![]);
    let user = tmp.0.join("user-config");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&user, "0.0.0-test").with_libraries(Some(tiers)),
    );
    drawn(&mut shell, "example.user", "User");
    done(
        &mut shell,
        "symbol.save",
        json!({"directory":data.join("libraries").join("symbols.ewlib")}),
    );
    shell.shutdown();
    let settings = std::fs::read_to_string(user.join("settings.toml")).unwrap_or_default();
    assert!(!settings.contains("company"), "{settings}");
}

#[test]
fn refusals_of_export_and_unknown_members_are_in_words() {
    let tmp = Tmp::new("symbol-refusals");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    drawn(&mut shell, "example.words", "Words");
    let file = json!({"file":tmp.0.join("words.symbol.json")});
    done(&mut shell, "symbol.export", file.clone());
    assert_eq!(
        failed(&mut shell, "symbol.export", file),
        "Symbol editor: A file with this name already exists. Choose another name."
    );
    let mut doc = serde_json::to_value(&shell.symbol_editor().unwrap().draft).unwrap();
    doc["symbol"]["text_slot"] = json!([]);
    done(&mut shell, "symbol.replace", json!({"document":doc}));
    assert_eq!(
        failed(
            &mut shell,
            "symbol.save",
            json!({"directory":tmp.0.join("symbols.ewlib")})
        ),
        "Symbol editor: The symbol has a field this version does not know: text_slot. Remove or rename it before \
         saving."
    );
    assert!(!tmp.0.join("symbols.ewlib").exists());
    shell.shutdown();
}

/// ID-1463 (M25-p3#5): Close leaves the view and keeps the draft with its history; Edit ▸ Symbol editor returns to it;
/// only Discard drops it.
#[test]
fn closing_the_editor_keeps_the_draft_and_its_history_until_discarded() {
    let tmp = Tmp::new("symbol-keep");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    drawn(&mut shell, "example.kept", "Kept");
    let draft = shell.symbol_editor().unwrap().draft.clone();
    done(&mut shell, "symbol.close", json!({}));
    assert!(shell.symbol_editor().is_none());
    assert!(shell.kept_symbol_editor().unwrap().dirty());
    done(&mut shell, "symbol.new", json!({}));
    let editor = shell.symbol_editor().unwrap();
    assert_eq!(editor.draft, draft);
    assert!(editor.can_undo());
    done(&mut shell, "edit.undo", json!({}));
    assert!(shell.symbol_editor().unwrap().draft.symbol.cps.is_empty());
    done(&mut shell, "symbol.close", json!({"discard": true}));
    assert!(shell.kept_symbol_editor().is_none());
    done(&mut shell, "symbol.new", json!({}));
    assert!(shell.symbol_editor().unwrap().draft.symbol.graphics.is_empty());
    shell.shutdown();
}

/// M25-p3#7: an unedited new, forked or imported draft is no unsaved change, and an import is in no library yet.
#[test]
fn the_draft_state_says_unsaved_only_after_an_edit_and_saved_only_once_stored() {
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-state");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let words = |shell: &ShellModel| strings::text(shell.symbol_editor().unwrap().state_key()).to_owned();
    done(
        &mut shell,
        "symbol.new",
        json!({"key":"example.state","name":"State"}),
    );
    assert!(!shell.symbol_editor().unwrap().dirty());
    assert_eq!(words(&shell), "Not in a library yet");
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4}),
    );
    assert_eq!(words(&shell), "Unsaved changes");
    let file = tmp.0.join("state.symbol.json");
    done(&mut shell, "symbol.export", json!({"file":file}));
    assert_eq!(words(&shell), "Saved");
    done(&mut shell, "symbol.close", json!({"discard": true}));
    done(&mut shell, "symbol.open", json!({"file":file}));
    assert!(!shell.symbol_editor().unwrap().dirty());
    assert_eq!(words(&shell), "Not in a library yet");
    let qet = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ew-import-qet/tests/fixtures/contact_no.elmt"
    );
    done(&mut shell, "symbol.open", json!({"file":qet}));
    assert_eq!(words(&shell), "Not in a library yet");
    done(
        &mut shell,
        "symbol.fork",
        json!({"source":"ew.iec.contact-no","key":"example.fork"}),
    );
    assert!(!shell.symbol_editor().unwrap().dirty());
    shell.shutdown();
}

/// M25-p3#11: an empty path is refused in words before any file system call, and a relative one resolves against the
/// default user library's folder, never the working directory.
#[test]
fn symbol_paths_are_refused_when_empty_and_resolve_against_the_user_library_folder() {
    let tmp = Tmp::new("symbol-paths");
    let config = tmp.0.join("config");
    let mut shell = ShellModel::new(Mode::Headless, ShellConfig::in_dir(&config, "0.0.0-test"));
    assert_eq!(
        failed(&mut shell, "symbol.open", json!({"file":""})),
        "Symbol editor: Choose a symbol file first."
    );
    drawn(&mut shell, "example.paths", "Paths");
    assert_eq!(
        failed(&mut shell, "symbol.export", json!({"file":"  "})),
        "Symbol editor: Choose a symbol file first."
    );
    assert_eq!(
        failed(&mut shell, "symbol.directory", json!({"directory":""})),
        "Symbol editor: Choose a library folder before saving."
    );
    done(&mut shell, "symbol.directory", json!({"directory":"mine.ewlib"}));
    let user = config.join("libraries");
    assert_eq!(
        std::path::PathBuf::from(&shell.symbol_editor().unwrap().directory),
        std::path::absolute(user.join("mine.ewlib")).unwrap()
    );
    done(&mut shell, "symbol.save", json!({}));
    assert!(user.join("mine.ewlib").is_dir());
    done(&mut shell, "symbol.export", json!({"file":"paths.symbol.json"}));
    assert!(user.join("paths.symbol.json").is_file());
    shell.shutdown();
}

/// M25-p3#11: in the window a missing path asks the house picker.
#[test]
fn the_window_asks_the_pickers_for_a_missing_symbol_path() {
    use ew_viewmodel::shell::Prompt;
    let tmp = Tmp::new("symbol-pickers");
    let mut shell = ShellModel::new(
        Mode::Gui,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "symbol.new",
        json!({"key":"example.pick","name":"Pick"}),
    );
    for (action, prompt) in [
        ("symbol.open", Prompt::OpenSymbol),
        ("symbol.export", Prompt::SaveSymbol),
        ("symbol.directory", Prompt::LibraryFolder),
    ] {
        done(&mut shell, action, json!({}));
        let asked = shell.take_prompt().unwrap();
        assert_eq!((asked.action, asked.prompt), (action, prompt));
    }
    shell.shutdown();
}

/// ID-1463: a test placement shows the page with the symbol selected and offers the way back to the kept draft.
#[test]
fn a_test_placement_shows_the_page_and_its_toast_returns_to_the_editor() {
    let tmp = Tmp::new("symbol-back");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "file.new",
        json!({"dir":tmp.0.join("project.ew"),"template":"builtin"}),
    );
    drawn(&mut shell, "example.back", "Back");
    done(&mut shell, "symbol.scratch", json!({}));
    assert!(shell.symbol_editor().is_none());
    assert_eq!(shell.selection().items.len(), 1);
    let toast = shell.notifications().newest_visible().unwrap().clone();
    let page = shell.page_labels()[shell.page_index()].clone();
    assert_eq!(toast.text, format!("Test copy placed on page {page}."));
    assert_eq!(
        toast.action(),
        Some(ew_viewmodel::notifications::NoteAction::BackToEditor)
    );
    done(
        &mut shell,
        "notifications.run",
        json!({"entry": toast.serial, "action": "edit"}),
    );
    assert_eq!(shell.symbol_editor().unwrap().draft.key, "example.back");
    done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
    assert_eq!(
        shell.notifications().newest_visible().unwrap().text,
        "Test copy placed on this page."
    );
    shell.shutdown();
}

/// M25-p3#6: the test copy's key is derived from its content, so placing again after a reopen embeds a new copy.
#[test]
fn a_test_copy_placed_again_after_reopening_the_project_takes_its_own_key() {
    let tmp = Tmp::new("symbol-reopen");
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test");
    let dir = tmp.0.join("project.ew");
    let mut shell = ShellModel::new(Mode::Headless, config.clone());
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "example.reopen", "Reopen");
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let mut shell = ShellModel::new(Mode::Headless, config);
    done(&mut shell, "file.open", json!({"dir":dir}));
    drawn(&mut shell, "example.reopen", "Reopen");
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":4,"y":0,"x2":4,"y2":4}),
    );
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = ew_viewmodel::ew_api::Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    let keys: std::collections::BTreeSet<String> = session
        .query()
        .symbols()
        .iter()
        .filter_map(|(_, s)| s.item.meta.key.as_ref().map(ToString::to_string))
        .filter(|k| k.starts_with("example.reopen.test."))
        .collect();
    assert_eq!(keys.len(), 2, "{keys:?}");
}

/// ID-1463: quitting with an unsaved draft asks Save symbol, Discard or Cancel; `discard` discards it.
#[test]
fn quitting_with_an_unsaved_draft_asks_and_discard_discards_it() {
    use ew_viewmodel::dialogs::spec;
    use ew_viewmodel::shell::{ConfirmAnswer, Dialog};
    let tmp = Tmp::new("symbol-quit");
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test");
    let mut shell = ShellModel::new(Mode::Headless, config.clone());
    drawn(&mut shell, "example.quit", "Quit block");
    done(&mut shell, "symbol.close", json!({}));
    assert_eq!(
        failed(&mut shell, "app.quit", json!({})),
        "This symbol has unsaved changes. Save it or choose Discard changes."
    );
    done(&mut shell, "app.quit", json!({"discard": true}));
    assert!(shell.should_quit() && shell.kept_symbol_editor().is_none());
    let mut shell = ShellModel::new(Mode::Gui, config);
    drawn(&mut shell, "example.quit", "Quit block");
    done(&mut shell, "app.quit", json!({}));
    let dialog = shell.dialog().unwrap().clone();
    assert!(matches!(dialog, Dialog::ConfirmDiscard { symbol: true, .. }));
    let s = spec(&dialog, &shell);
    assert_eq!(s.title, "Save the symbol Quit block?");
    assert_eq!(
        s.body,
        [
            "The symbol draft has changes that are not in a library. Save it before you quit, or discard the changes."
        ]
    );
    let labels: Vec<&str> = s.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Save symbol", "Cancel", "Discard changes"]);
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Cancel), Outcome::Done);
    assert!(!shell.should_quit() && shell.symbol_editor().is_some());
    done(&mut shell, "app.quit", json!({}));
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Discard), Outcome::Done);
    assert!(shell.should_quit() && shell.kept_symbol_editor().is_none());
    shell.shutdown();
}

/// ID-1463: while the editor shows, Ctrl+S saves the symbol and the File menu names Save symbol and keeps Save project.
#[test]
fn the_file_menu_names_save_symbol_and_keeps_save_project_while_the_editor_shows() {
    use ew_viewmodel::actions::{MenuEntry, menu_model};
    let tmp = Tmp::new("symbol-menu");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
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
    drawn(&mut shell, "example.menu", "Menu");
    assert_eq!(labels(&shell), ["Save symbol", "Save project", "Save as…"]);
    done(&mut shell, "symbol.close", json!({}));
    done(&mut shell, "page.new", json!({"folio":"7"}));
    done(&mut shell, "symbol.new", json!({}));
    assert!(shell.is_dirty());
    done(&mut shell, "file.save_project", json!({}));
    assert!(!shell.is_dirty());
    done(&mut shell, "symbol.directory", json!({"directory":"menu.ewlib"}));
    done(&mut shell, "file.save", json!({}));
    assert!(shell.symbol_editor().unwrap().stored());
    shell.shutdown();
}

/// M25-p5#5: the file picker offers exactly the extensions the editor reads: JSON, and every extension an importer
/// takes (a file the JSON reader would refuse otherwise), `.pxf` among them.
#[test]
fn the_symbol_file_picker_offers_every_extension_an_importer_reads() {
    use ew_viewmodel::ew_api::symbols;
    use ew_viewmodel::shell::symbol_editor::FILE_EXTENSIONS;
    let tmp = Tmp::new("symbol-extensions");
    let refusal = |ext: &str| {
        let path = tmp.0.join(format!("probe.{ext}"));
        std::fs::write(&path, b"not a symbol").unwrap();
        symbols::open(&path, "example.probe").unwrap_err().to_string()
    };
    let json = refusal("json");
    let read: Vec<&str> = [
        "elmt", "edz", "pxf", "ema", "ems", "esl", "slk", "xml", "zip", "qet", "txt", "dxf",
    ]
    .into_iter()
    .filter(|ext| refusal(ext) != json)
    .collect();
    let offered: Vec<&str> = FILE_EXTENSIONS.into_iter().filter(|e| *e != "json").collect();
    assert_eq!(offered, read);
    assert!(FILE_EXTENSIONS.contains(&"json"));
}

/// ID-1463 (M25-p5 nit): the quit question's Save symbol saves the draft, then quits.
#[test]
fn the_quit_questions_save_symbol_saves_the_draft_and_quits() {
    use ew_viewmodel::shell::ConfirmAnswer;
    let tmp = Tmp::new("symbol-quit-save");
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test");
    let mut shell = ShellModel::new(Mode::Gui, config);
    drawn(&mut shell, "example.quit-save", "Quit save");
    done(&mut shell, "symbol.directory", json!({"directory": "quit.ewlib"}));
    done(&mut shell, "app.quit", json!({}));
    assert!(shell.dialog().is_some() && !shell.should_quit());
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Save), Outcome::Done);
    assert!(shell.should_quit());
    assert!(shell.kept_symbol_editor().unwrap().stored());
    assert!(tmp.0.join("config").join("libraries").join("quit.ewlib").is_dir());
    shell.shutdown();
}

/// The quit question's and the scratch page's titles name the symbol in the window's language.
#[test]
fn the_titles_name_the_symbol_in_the_windows_language() {
    use ew_viewmodel::dialogs::spec;
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-language");
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test");
    let dir = tmp.0.join("project.ew");
    let mut shell = ShellModel::new(Mode::Gui, config);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "example.language", "Supply block");
    let mut named = serde_json::to_value(&shell.symbol_editor().unwrap().draft).unwrap();
    named["symbol"]["name"] = json!({"en": "Supply block", "nl": "Voedingsblok"});
    done(&mut shell, "symbol.replace", json!({"document": named}));
    assert!(strings::set_locale("nl"));
    done(&mut shell, "app.quit", json!({}));
    let title = spec(shell.dialog().unwrap(), &shell).title;
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "symbol.scratch", json!({}));
    assert!(strings::set_locale("en"));
    assert_eq!(title, "Symbool Voedingsblok opslaan?");
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = ew_viewmodel::ew_api::Session::open_read_only(&dir, "ElektraWatt test").unwrap();
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

/// Part 3's nit (D6): `symbol.role` sets a connection point's potential role, `none` clears it, each one undo step;
/// a missing point and an assigned function definition are refused in words.
#[test]
fn symbol_role_sets_and_clears_a_connection_points_potential_role() {
    let tmp = Tmp::new("symbol-role");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    drawn(&mut shell, "example.role", "Role");
    let role = |shell: &ShellModel| {
        let draft = &shell.symbol_editor().unwrap().draft;
        serde_json::to_value(draft).unwrap()["function"]["cps"][0]["potential_role"].clone()
    };
    done(&mut shell, "symbol.role", json!({"cp": 1, "role": "fe"}));
    assert_eq!(role(&shell), json!("fe"));
    done(&mut shell, "symbol.role", json!({"cp": 1, "role": "l_plus"}));
    assert_eq!(role(&shell), json!("l_plus"));
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(role(&shell), json!("fe"));
    done(&mut shell, "symbol.role", json!({"cp": 1, "role": "none"}));
    assert_eq!(role(&shell), serde_json::Value::Null);
    assert_eq!(
        failed(&mut shell, "symbol.role", json!({"cp": 9, "role": "pe"})),
        "Symbol editor: This symbol has no connection point 9."
    );
    done(&mut shell, "symbol.function", json!({"key":"ew.fn.contact-no"}));
    assert_eq!(
        failed(&mut shell, "symbol.role", json!({"cp": 1, "role": "pe"})),
        "Symbol editor: An assigned function definition keeps its roles. Change them in a function definition of the \
         symbol's own."
    );
    shell.shutdown();
}

/// Part 3's nits: a refused action names no member ("index", "idx", "start"), and an Image primitive has its word.
#[test]
fn refusals_of_the_editors_actions_are_in_words() {
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-words");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    drawn(&mut shell, "example.words", "Words");
    for (action, params, text) in [
        (
            "symbol.candidate",
            json!({"index": 5, "discard": true}),
            "Choose one of the drawings this file holds.",
        ),
        ("symbol.variant", json!({"variant": 9}), "Choose a variant from A to H."),
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
        assert_eq!(
            failed(&mut shell, action, params),
            format!("Symbol editor: {text}"),
            "{action}"
        );
    }
    assert_eq!(strings::text(strings::Key::run_time("symbol-image")), "Image");
    shell.shutdown();
}

/// Re-review part 4: the same content placed twice is one embedded copy, placed twice.
#[test]
fn the_same_content_placed_twice_reuses_one_copy() {
    let tmp = Tmp::new("symbol-twice");
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test");
    let dir = tmp.0.join("project.ew");
    let mut shell = ShellModel::new(Mode::Headless, config);
    done(&mut shell, "file.new", json!({"dir":dir,"template":"builtin"}));
    drawn(&mut shell, "example.twice", "Twice");
    done(&mut shell, "symbol.scratch", json!({}));
    done(&mut shell, "symbol.new", json!({}));
    done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = ew_viewmodel::ew_api::Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    let q = session.query();
    let copies: Vec<_> = q
        .symbols()
        .iter()
        .filter(|(_, s)| {
            s.item
                .meta
                .key
                .as_ref()
                .is_some_and(|k| k.to_string().starts_with("example.twice.test."))
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(copies.len(), 1, "{copies:?}");
    let placed = q.reps().iter().filter(|(_, r)| r.symbol == copies[0]).count();
    assert_eq!(placed, 2);
}

/// M25-p4#6: the library writer's version refusals reach the window in words, in both languages: the symbol by its
/// name, the versions, no UUID.
#[test]
fn version_refusals_of_a_save_name_the_symbol_and_its_versions() {
    use ew_viewmodel::strings;
    let tmp = Tmp::new("symbol-versions");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    drawn(&mut shell, "example.versions", "Versions block");
    let library = json!({"directory": tmp.0.join("versions.ewlib")});
    done(&mut shell, "symbol.save", library.clone());
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"line","x":4,"y":0,"x2":4,"y2":4}),
    );
    assert_eq!(
        failed(&mut shell, "symbol.save", library.clone()),
        "Symbol editor: The library holds the symbol Versions block at version 1.0.0 with other content. Give the \
         change a new version."
    );
    assert!(strings::set_locale("nl"));
    let dutch = failed(&mut shell, "symbol.save", library.clone());
    assert!(strings::set_locale("en"));
    assert_eq!(
        dutch,
        "Symbooleditor: De bibliotheek bevat het symbool Versions block al in versie 1.0.0 met andere inhoud. Geef de \
         wijziging een nieuwe versie."
    );
    let mut older = serde_json::to_value(&shell.symbol_editor().unwrap().draft).unwrap();
    older["version"] = json!("0.9.0");
    done(&mut shell, "symbol.replace", json!({"document": older}));
    assert_eq!(
        failed(&mut shell, "symbol.save", library),
        "Symbol editor: The library holds the symbol Versions block at the newer version 1.0.0; version 0.9.0 would \
         replace it. Give the change a version above 1.0.0."
    );
    shell.shutdown();
}

/// ID-1472 (M25-p5#6): `symbol.new` with `discard` starts a fresh draft; without a key or name it returns to the kept
/// one.
#[test]
fn a_new_symbol_with_discard_starts_a_fresh_draft() {
    let tmp = Tmp::new("symbol-fresh");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    drawn(&mut shell, "example.kept", "Kept");
    done(&mut shell, "symbol.close", json!({}));
    done(&mut shell, "symbol.new", json!({}));
    assert_eq!(shell.symbol_editor().unwrap().draft.key, "example.kept");
    done(&mut shell, "symbol.new", json!({"discard": true}));
    let editor = shell.symbol_editor().unwrap();
    assert!(editor.draft.symbol.graphics.is_empty() && editor.draft.key.is_empty());
    assert!(!editor.dirty() && !editor.can_undo());
    shell.shutdown();
}

/// ID-1472 (M25-p5#6 and the discard question's nit): in the window, New symbol over unsaved changes asks Save
/// symbol, Discard or Cancel in the house dialog, in words that match its buttons; a clean draft is replaced at once.
#[test]
fn new_symbol_over_unsaved_changes_asks_save_discard_or_cancel() {
    use ew_viewmodel::dialogs::spec;
    use ew_viewmodel::shell::{ConfirmAnswer, Dialog};
    let tmp = Tmp::new("symbol-ask");
    let mut shell = ShellModel::new(Mode::Gui, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
    drawn(&mut shell, "example.ask", "Ask block");
    let fresh = json!({"discard": true});
    assert_eq!(shell.ask_symbol_discard("symbol.new", fresh.clone()), Outcome::Done);
    let dialog = shell.dialog().unwrap().clone();
    assert!(matches!(dialog, Dialog::ConfirmDiscard { symbol: true, .. }));
    let s = spec(&dialog, &shell);
    assert_eq!(s.title, "Save the symbol Ask block?");
    assert_eq!(
        s.body,
        ["The symbol draft has changes that are not in a library. Save it first, or discard the changes."]
    );
    let labels: Vec<&str> = s.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Save symbol", "Cancel", "Discard changes"]);
    // Cancel keeps the draft.
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Cancel), Outcome::Done);
    assert_eq!(shell.symbol_editor().unwrap().draft.key, "example.ask");
    // Discard starts the fresh draft.
    assert_eq!(shell.ask_symbol_discard("symbol.new", fresh.clone()), Outcome::Done);
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Discard), Outcome::Done);
    assert!(shell.symbol_editor().unwrap().draft.key.is_empty());
    // Save symbol saves the draft first, then starts the fresh one.
    drawn(&mut shell, "example.ask", "Ask block");
    done(&mut shell, "symbol.directory", json!({"directory": "ask.ewlib"}));
    assert_eq!(shell.ask_symbol_discard("symbol.new", fresh.clone()), Outcome::Done);
    assert_eq!(shell.answer_confirm(ConfirmAnswer::Save), Outcome::Done);
    assert!(shell.dialog().is_none());
    assert!(shell.symbol_editor().unwrap().draft.key.is_empty());
    assert!(tmp.0.join("config").join("libraries").join("ask.ewlib").is_dir());
    // A clean draft asks nothing.
    assert_eq!(shell.ask_symbol_discard("symbol.new", fresh), Outcome::Done);
    assert!(shell.dialog().is_none());
    shell.shutdown();
}
