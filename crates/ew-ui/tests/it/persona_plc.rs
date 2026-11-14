// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! M3's persona round on a new project, by the window's own path: what a first-timer and an EPLAN designer could not
//! find or do when they built a PLC and a fieldbus from nothing.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use ew_ui::visual::{Driver, Input};
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

/// Three of three personas looked for a list page and the PLC exchange on the Output tab: it holds Report (the report
/// block dialog), Export to PLC software and Import from PLC software; the command search finds the report dialog by
/// "report", "list", "tube list" and "card list"; and the New page dialog says where lists are made.
#[test]
fn lists_and_the_plc_exchange_are_found_on_the_output_tab() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Output").expect("the Output tab")));
    d.settle();
    for button in ["Export to PLC software", "Import from PLC software"] {
        assert!(d.find(button).is_some(), "{button} on the Output tab");
    }
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
        texts
            .iter()
            .any(|t| t.starts_with("Lists (parts, wires, terminals") && t.contains("Report on the Output tab")),
        "New page says where lists are made: {texts:?}"
    );
    d.shell_mut().shutdown();
}
