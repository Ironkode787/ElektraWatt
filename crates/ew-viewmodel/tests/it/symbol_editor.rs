// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

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
        done(&mut shell, "symbol.place", json!({"x":80,"y":80}));
        done(
            &mut shell,
            "symbol.draw",
            json!({"tool":"line","x":1,"y":1,"x2":2,"y2":2}),
        );
        done(&mut shell, "symbol.place", json!({"x":120,"y":80}));
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
