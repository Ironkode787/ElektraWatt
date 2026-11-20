// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! M3's persona round on a new project, by the window's own path: what a first-timer and an EPLAN designer could not
//! find or do when they built a PLC and a fieldbus from nothing.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::visual::{Driver, Input};
use ew_viewmodel::shell::{Dialog, Outcome};
use serde_json::json;

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

/// A driver over a new project from the built-in template, at 1920 × 1080.
fn new_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// The action IDs the command search lists for `query`.
fn search(d: &mut Driver, query: &str) -> Vec<String> {
    let _ = d.shell_mut().dispatch("tools.palette", &json!({}));
    d.shell_mut().set_palette_query(query);
    let ids = d
        .shell_mut()
        .palette_entries()
        .into_iter()
        .map(|e| e.id)
        .collect();
    let _ = d.shell_mut().dispatch("ui.dismiss", &json!({}));
    d.settle();
    ids
}

/// Import from PLC software… on the PLC tab with a file chosen opens its preview; with the file dialog cancelled
/// (the persona's run answered every open dialog so) nothing changes and nothing opens, as a cancelled dialog does
/// everywhere in the window.
#[test]
fn import_from_plc_software_previews_a_chosen_file_and_a_cancel_changes_nothing() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    d.set_script_dir(tmp.0.clone());
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    let file = tmp.0.join("back.aml");
    let export = json!({"format": "aml-1.4.0", "path": file.to_string_lossy()});
    assert_eq!(d.shell_mut().dispatch("plc.export", &export), Outcome::Done);
    d.settle();
    // The first widget named PLC is the command bar's tab.
    d.input(&click(d.find("PLC").expect("the PLC tab")));
    d.settle();
    let before = d.shell_mut().revision();
    d.input(&click(d.find("Import from PLC").expect("the button")));
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "a cancel opens nothing");
    assert_eq!(d.shell_mut().banner(), None, "and says nothing");
    assert_eq!(d.shell_mut().revision(), before, "and changes nothing");
    d.input(&Input::AnswerOpen {
        path: "back.aml".to_owned(),
    });
    d.input(&click(d.find("Import from PLC").expect("the button")));
    d.settle();
    assert!(
        matches!(d.shell_mut().dialog(), Some(Dialog::PlcImport(_))),
        "the chosen file's preview: {:?}",
        d.shell_mut().dialog()
    );
    d.shell_mut().shutdown();
}

/// Three of three personas looked for a list page on the Output tab: it holds Report (the report block dialog); the
/// PLC exchange, which they looked for there too, moved to the PLC tab with the other PLC commands (the owner's test
/// of 0.3.0-rc.1); the command search finds the report dialog by "report", "list", "tube list" and "card list"; and the
/// New page dialog says where lists are made.
#[test]
fn lists_are_found_on_the_output_tab_and_the_plc_exchange_on_the_plc_tab() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("PLC").expect("the PLC tab")));
    d.settle();
    for button in ["Export to PLC", "Import from PLC"] {
        assert!(d.find(button).is_some(), "{button} on the PLC tab");
    }
    d.input(&click(d.find("Output").expect("the Output tab")));
    d.settle();
    d.input(&click(d.find("Report").expect("Report on the Output tab")));
    d.settle();
    assert!(
        matches!(d.shell_mut().dialog(), Some(Dialog::Form(f)) if f.action == "report_block.new"),
        "Report opens the report block dialog"
    );
    let _ = d.shell_mut().dispatch("ui.dismiss", &json!({}));
    d.settle();
    for query in ["report", "list", "tube list", "card list"] {
        assert!(
            search(&mut d, query).iter().any(|id| id == "report_block.new"),
            "{query:?} finds Report block…"
        );
    }
    let _ = d.shell_mut().dispatch("page.new", &json!({}));
    d.settle();
    // The dialog fades in over `motion.base`.
    d.input(&Input::Wait { ms: 300 });
    assert!(
        matches!(d.shell_mut().dialog(), Some(Dialog::Form(f)) if f.action == "page.new"),
        "{:?}",
        d.shell_mut().dialog()
    );
    let texts = d.texts();
    assert!(
        // Under the default type's words (the persona run m3-p4: the line says what the chosen type is for).
        texts
            .iter()
            .any(|t| t.contains("Lists are made with Report on the Output tab")),
        "New page says where lists are made: {texts:?}"
    );
    d.shell_mut().shutdown();
}
