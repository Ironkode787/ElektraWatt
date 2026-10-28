// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Follow-ups of the first blind-drafter dry run (BD16) found while fixing FT-485 (cycle 30, lane O30): the two
//! clamps of a starter terminal print distinct labels, the strip chooser's `picker.choose` takes its own
//! `designation`, and the keyboard move of a terminal placed through the text interface moves it by the grid
//! cursor's displacement.

mod common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a new project (the builtin template), a new folio 101 shown and one new strip.
fn new_project_with_strip(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "strip.new", json!({"letter": "X"}));
    s
}

/// Places a strip terminal at cell `cell` through the symbol picker and the strip chooser: `first` are the
/// parameters of the symbol row's choice, `second` those of the strip row's.
fn place_terminal(s: &mut ShellModel, cell: &str, first: Value, second: Value) {
    done(s, "tool.place", json!({}));
    done(s, "picker.filter", json!({"text": "iec.terminal@"}));
    done(s, "picker.choose", first);
    done(s, "picker.choose", second);
    done(s, "nav.goto", json!({ "text": cell }));
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "ui.dismiss", json!({}));
}

/// The labels of the page's connection points, sorted.
fn cp_labels(s: &ShellModel) -> Vec<String> {
    let mut v: Vec<String> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .cps
        .iter()
        .map(|c| c.label.clone())
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------------------------------------------
// Item 1: the two clamps of a terminal print distinct labels.

#[test]
fn item1_the_two_clamps_of_a_starter_terminal_print_distinct_labels() {
    let tmp = Tmp::new("bd16-1");
    let mut s = new_project_with_strip(&tmp);
    place_terminal(&mut s, "101-D2", json!({}), json!({}));
    let labels = cp_labels(&s);
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert_ne!(labels[0], labels[1], "{labels:?}");
    // `ew.fn.terminal` names clamp `a` on both sides: the label takes `#n` in CP index order (side 1, then side 2).
    assert!(labels[0].ends_with(":1/a#1"), "{labels:?}");
    assert!(labels[1].ends_with(":1/a#2"), "{labels:?}");
}
