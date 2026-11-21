// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The readout of a move says what the move does on the page: a symbol fitted to a page of the 4 mm grid, moved by the
//! arrow key, reads the whole step the move then makes, not the part of it the sheet's smaller drawing of the page
//! shows.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The sheet point of the page point `p`.
fn sheet(shell: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    shell.canvas().unwrap().page().snapper.page_to_sheet(p)
}

#[test]
fn the_first_arrow_key_on_a_fitted_symbol_reads_the_step_it_moves() {
    let tmp = Tmp::new("fitted-readout");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    // A terminal of folio 101 (its position a step above its first pin), drawn on the QET grid, pasted onto a page of
    // the 4 mm grid: its first pin on the grid, its position between grid points.
    done(&mut shell, "select.set", json!({"refs": ["-X1011:L1"]}));
    done(&mut shell, "edit.copy", json!({}));
    done(
        &mut shell,
        "page.new",
        json!({"folio": "150", "title": "Fit", "grid": "mm4"}),
    );
    done(&mut shell, "edit.paste_clip", json!({}));
    let target = sheet(&shell, (60_000_000, 60_000_000));
    done(&mut shell, "canvas.pointer", at(target, json!({})));
    done(&mut shell, "canvas.press", at(target, json!({})));
    let rep = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.pos.0 % 4_000_000 != 0 || r.pos.1 % 4_000_000 != 0)
        .expect("a symbol with its position between grid points")
        .clone();
    done(&mut shell, "select.set", json!({ "ids": [rep.id.to_string()] }));
    done(&mut shell, "tool.move", json!({}));
    done(&mut shell, "cursor.down", json!({}));
    let labels: Vec<String> = shell.tool_overlay().labels().map(str::to_owned).collect();
    assert_eq!(labels, ["dx 0.000 dy 4.000 mm"]);
    done(&mut shell, "canvas.activate", json!({}));
    let after = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.id == rep.id)
        .expect("the symbol")
        .pos;
    assert_eq!(
        (after.0 - rep.pos.0, after.1 - rep.pos.1),
        (0, 4_000_000),
        "the move"
    );
}
