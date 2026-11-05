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
        done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool":"line","x":1,"y":1,"x2":2,"y2":2}),
        );
        done(&mut shell, "symbol.place", json!({"x":120,"y":80}));
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
        assert_eq!(
            shell.dispatch("symbol.close", &json!({})),
            Outcome::Failed {
                message: "Symbol editor: This symbol has unsaved changes. Save it or choose Discard changes."
                    .into()
            }
        );
        done(&mut shell, "symbol.close", json!({"discard":true}));
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
