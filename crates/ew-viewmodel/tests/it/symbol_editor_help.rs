// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F1 in the symbol editor opens the section of what is selected: a connection point selected, F1 over the
//! outline or the drawing opens the guide's section on connection points, not the outline's.

use crate::common::{Tmp, done};
use ew_viewmodel::help::{EditorPart, symbol_editor_part};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn f1_on_a_selected_connection_point_opens_its_section() {
    let tmp = Tmp::new("symbol-editor-help");
    let mut shell = tmp.empty_shell(Mode::Headless);
    crate::common::new_drawing(&mut shell, "Help");
    done(
        &mut shell,
        "symbol.cp",
        json!({"x": 0, "y": 4, "designation": "1", "direction": "left"}),
    );
    done(&mut shell, "symbol.select", json!({"items": ["cp:0"]}));
    let editor = shell.symbol_editor().expect("the editor");
    for part in [EditorPart::Outline, EditorPart::Drawing] {
        assert_eq!(symbol_editor_part(editor, part), "symbol-editor-cp", "{part:?}");
    }
    done(&mut shell, "symbol.select", json!({"items": []}));
    let editor = shell.symbol_editor().expect("the editor");
    assert_eq!(
        symbol_editor_part(editor, EditorPart::Outline),
        "symbol-editor-outline"
    );
}
