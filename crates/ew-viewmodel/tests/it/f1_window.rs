// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F1 in every part of the window (WP-S7.05 D8, DC-3) on a copy of Golden A: a focused navigator, the strip editor, a
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
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "devices"}),
    );
    assert_eq!(f1(&mut shell), "reference/window/panels.html#devices");
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
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    done(&mut shell, "ui.focus", json!({"area": "properties"}));
    done(&mut shell, "nav.goto", json!({}));
    assert_eq!(f1(&mut shell), "reference/window/dialogs.html#go-to-page-or-cell");
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "page.new", json!({}));
    assert_eq!(f1(&mut shell), "reference/window/dialogs.html#form-dialogs");
    shell.shutdown();
}
