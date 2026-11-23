// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The spacing of a terminal row is kept between sessions (FT-1791): a row laid four grid steps apart, the program
//! closed and the project opened again, the next row takes four steps without being told. Untold, a row takes three.

use crate::common::{self, Tmp, done};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The grid steps between the terminals of the page shown that `before` does not hold, left to right.
fn new_gaps(s: &ShellModel, before: &[ew_api::output::Uuid]) -> Vec<i64> {
    let page = s.canvas().unwrap().page();
    let mut x: Vec<i64> = page
        .tables
        .reps
        .iter()
        .filter(|r| r.terminal && !before.contains(&r.id))
        .map(|r| page.snapper.grid_index_of(r.pos.0))
        .collect();
    x.sort_unstable();
    x.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn the_next_terminal_row_takes_the_spacing_of_an_earlier_session() {
    let tmp = Tmp::new("terminal-spacing-kept");
    let mut s = common::new_project(&tmp);
    done(
        &mut s,
        "tool.place_terminal",
        json!({"strip": "new", "count": 2, "spacing": 4}),
    );
    done(&mut s, "nav.goto", json!({ "text": "101-D2" }));
    done(&mut s, "canvas.activate", json!({"alt": true}));
    assert_eq!(new_gaps(&s, &[]), [4]);
    done(&mut s, "file.save", json!({}));
    s.shutdown();

    let mut s = tmp.empty_shell(Mode::Headless);
    done(
        &mut s,
        "file.open",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy()}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let before: Vec<_> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id)
        .collect();
    done(&mut s, "tool.place_terminal", json!({"strip": "new", "count": 2}));
    done(&mut s, "nav.goto", json!({ "text": "101-F2" }));
    done(&mut s, "canvas.activate", json!({"alt": true}));
    assert_eq!(new_gaps(&s, &before), [4], "the spacing of the earlier session");
    s.shutdown();
}

/// Untold, a row lays its terminals three grid steps apart, the value the Terminal… form opens on (three persona runs
/// at two found the numbers against the neighbours' leads); the choice of two stays in the list.
#[test]
fn a_row_untold_lays_its_terminals_three_grid_steps_apart() {
    let tmp = Tmp::new("terminal-spacing-default");
    let mut s = common::new_project(&tmp);
    done(&mut s, "tool.place_terminal", json!({"strip": "new", "count": 2}));
    done(&mut s, "nav.goto", json!({ "text": "101-D2" }));
    done(&mut s, "canvas.activate", json!({"alt": true}));
    assert_eq!(new_gaps(&s, &[]), [3]);
}
