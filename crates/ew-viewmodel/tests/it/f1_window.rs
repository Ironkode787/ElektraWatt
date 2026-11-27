// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F1 in every part of the window on a copy of the sample: a focused navigator, the strip editor, a
//! navigator in table mode, the property grid, the find pane, the history, the command bar, an active tool and the
//! dialogs open the section of their page of the window reference.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The page F1 opens now.
fn f1(shell: &mut ShellModel) -> String {
    done(shell, "help.documentation", json!({}));
    shell.last_help().unwrap().to_owned()
}

#[test]
fn f1_opens_the_section_of_the_focused_pane() {
    let tmp = Tmp::new("f1-panes");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "devices"}),
    );
    // The cursor row is a device: what devices are.
    assert_eq!(f1(&mut shell), "reference/window/things.html#devices-and-symbols");
    done(&mut shell, "navigator.table", json!({"kind": "devices"}));
    assert_eq!(f1(&mut shell), "reference/window/panels.html#table-mode");
    // FR-UI-031: F1 with the strip editor focused opens its page.
    done(&mut shell, "strip.edit", json!({"strip": "-X1031"}));
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "strips"}),
    );
    assert_eq!(f1(&mut shell), "reference/window/panels.html#strip-editor");
    done(&mut shell, "ui.focus", json!({"area": "properties"}));
    assert_eq!(f1(&mut shell), "reference/window/panels.html#properties");
    done(&mut shell, "find.open", json!({}));
    done(&mut shell, "ui.focus", json!({"area": "find"}));
    assert_eq!(f1(&mut shell), "reference/window/panels.html#find");
    done(&mut shell, "history.show", json!({"on": true}));
    done(&mut shell, "ui.focus", json!({"area": "history"}));
    assert_eq!(f1(&mut shell), "reference/window/panels.html#history");
    done(&mut shell, "ui.focus", json!({"area": "commandbar"}));
    assert_eq!(f1(&mut shell), "reference/window/panels.html#command-bar");
    // The canvas with a tool other than Select opens the tool's section; with Select, the project view's.
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    done(&mut shell, "tool.wire", json!({}));
    assert_eq!(f1(&mut shell), "reference/window/tools.html#wire");
    done(&mut shell, "tool.select", json!({}));
    assert_eq!(f1(&mut shell), "how-to/application-window.html#project-view");
    shell.shutdown();
}

/// A dialog's topic wins over the focus: the window's Go to page or cell and a form dialog.
#[test]
fn f1_in_a_dialog_opens_its_section() {
    let tmp = Tmp::new("f1-dialogs");
    let mut shell = tmp.sample_in(Mode::Gui, true, "102");
    done(&mut shell, "ui.focus", json!({"area": "properties"}));
    done(&mut shell, "nav.goto", json!({}));
    assert_eq!(f1(&mut shell), "reference/window/dialogs.html#go-to-page-or-cell");
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "page.new", json!({}));
    // A form opens its action's section: New page…'s on the command bar's page.
    assert_eq!(f1(&mut shell), "reference/window/command-bar.html#new-page");
    shell.shutdown();
}

/// F1 leads to the guide
/// of the thing in front: the PLC panel and the PLC tab to the PLC guide, the Bus panel and a bus-topology page to its
/// bus part, the Fluid panel, a fluid page and the Fluid tab to the fluid guide, a panel layout page to the panel
/// layout guide, Parts to "your own parts"; the guide buttons of the tabs open the same pages.
#[test]
fn f1_and_the_guide_buttons_lead_to_the_guide_of_the_discipline() {
    let tmp = Tmp::new("f1-guides");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    for (kind, page) in [
        ("plc", "how-to/plc-workspace.html"),
        ("plc_grid", "how-to/plc-workspace.html"),
        ("bus", "how-to/plc-workspace.html#bus-ports-and-nets"),
        ("fluid", "how-to/fluid-workspace.html"),
    ] {
        done(&mut shell, "navigator.show", json!({ "kind": kind }));
        done(&mut shell, "ui.focus", json!({"area": "navigator", "kind": kind}));
        assert_eq!(f1(&mut shell), page, "{kind}");
    }
    done(&mut shell, "ui.focus", json!({"area": "commandbar"}));
    for (tab, page) in [
        ("plc", "how-to/plc-workspace.html"),
        ("fluid", "how-to/fluid-workspace.html"),
        ("draw", "reference/window/panels.html#command-bar"),
    ] {
        done(&mut shell, "view.command_tab", json!({ "tab": tab }));
        assert_eq!(f1(&mut shell), page, "{tab}");
    }
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    for (folio, page) in [
        ("105", "how-to/plc-workspace.html#bus-ports-and-nets"),
        ("201", "how-to/fluid-workspace.html"),
    ] {
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        assert_eq!(f1(&mut shell), page, "page {folio}");
    }
    for (action, page) in [
        ("plc.guide", "how-to/plc-workspace.html"),
        ("fluid.guide", "how-to/fluid-workspace.html"),
    ] {
        done(&mut shell, action, json!({}));
        assert_eq!(shell.last_help(), Some(page), "{action}");
    }
    done(&mut shell, "parts.open", json!({ "part": "EW-GEN-MCB-1" }));
    assert_eq!(f1(&mut shell), "how-to/own-parts.html");
    shell.shutdown();
}

/// Help ▸ Guides lists the how-to guides of the book under a line that says what a guide is, and Help ▸ Tutorials
/// says what a tutorial is the same way; a guide opens its page without a question.
#[test]
fn help_guides_lists_the_how_to_guides_and_opens_one() {
    use ew_viewmodel::actions::{Menu, Submenu, menu_model};
    let tmp = Tmp::new("f1-guides-menu");
    let mut shell = tmp.empty_shell(Mode::Headless);
    let help = menu_model(&shell)
        .into_iter()
        .find(|m| m.menu == Menu::Help)
        .expect("the Help menu");
    let guides = help.submenu(Submenu::Guides).expect("Help ▸ Guides").to_vec();
    assert_eq!(
        (guides[0].label.as_str(), guides[0].enabled),
        ("How to do one job", false)
    );
    let plc = guides
        .iter()
        .find(|i| i.label == "Work in the PLC workspace")
        .expect("the PLC guide");
    let tutorials = help.submenu(Submenu::Tutorials).expect("Help ▸ Tutorials");
    assert_eq!(
        (tutorials[0].label.as_str(), tutorials[0].enabled),
        ("Lessons you follow step by step", false)
    );
    done(&mut shell, plc.id, plc.params.clone());
    assert_eq!(shell.last_help(), Some("how-to/plc-workspace.html"));
    assert!(shell.dialog().is_none());
    shell.shutdown();
}

/// F1 on a drawing does not open its general page whatever is in front. A selected shape and a selected connection
/// point open their own places; the bar's buttons open theirs (Highlight its own section, not the panels page).
#[test]
fn f1_opens_the_own_place_of_the_editors_parts_and_the_bars_buttons() {
    let tmp = Tmp::new("f1-own-places");
    let mut shell = tmp.empty_shell(Mode::Headless);
    crate::common::new_drawing(&mut shell, "F1");
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool": "rectangle", "x": 0, "y": 0, "x2": 8, "y2": 8}),
    );
    assert_eq!(f1(&mut shell), "reference/symbol-editor.html#a-shape");
    done(
        &mut shell,
        "symbol.cp",
        json!({"x": 0, "y": 0, "designation": "1"}),
    );
    assert_eq!(f1(&mut shell), "reference/symbol-editor.html#a-connection-point");
    done(&mut shell, "parts.back", json!({}));
    done(
        &mut shell,
        "help.documentation",
        json!({"topic": "navigator.highlight"}),
    );
    assert_eq!(
        shell.last_help(),
        Some("reference/window/command-bar.html#highlight")
    );
    shell.shutdown();
}
