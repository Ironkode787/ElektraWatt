// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

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
