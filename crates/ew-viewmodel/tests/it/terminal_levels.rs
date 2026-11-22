// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminals of two levels from the window (doc 08 §6.3): Terminal…'s Levels 2 lays each block as two symbols that are
//! one terminal of one part, a number on each level, proposing the starter's two-level part; one Undo takes the row
//! back. And the Join chooser of a terminal lists the free terminals of its strip only, the nearest first.

use std::collections::BTreeSet;

use crate::common;

use common::{Tmp, done, tables};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    s
}

/// Terminal… with `params`, then a click on an empty spot of the page.
fn place_row(s: &mut ShellModel, params: serde_json::Value) {
    done(s, "tool.place_terminal", params);
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(10), sn.page_of_index(20)))
    };
    common::click(s, p, json!({"alt": true}));
}

/// The page's terminal symbols as (device, symbol), left to right.
fn terminal_symbols(s: &ShellModel) -> Vec<(String, String)> {
    let mut reps: Vec<_> = tables(s)
        .reps
        .iter()
        .filter(|r| r.terminal)
        .map(|r| {
            (
                r.pos,
                r.device.map(|d| d.to_string()).unwrap_or_default(),
                r.id.to_string(),
            )
        })
        .collect();
    reps.sort();
    reps.into_iter().map(|(_, d, r)| (d, r)).collect()
}

/// The labels of the page's connection points, sorted.
fn cp_labels(s: &ShellModel) -> Vec<String> {
    let mut v: Vec<String> = tables(s).cps.iter().map(|c| c.label.clone()).collect();
    v.sort();
    v
}

#[test]
fn levels_two_lays_each_block_as_one_terminal_with_a_number_on_each_level() {
    let tmp = Tmp::new("terminal-levels");
    let mut s = new_project(&tmp);
    place_row(&mut s, json!({"strip": "new", "levels": "2", "count": "3"}));
    let symbols = terminal_symbols(&s);
    assert_eq!(symbols.len(), 6, "two symbols a block: {symbols:?}");
    let devices: BTreeSet<&str> = symbols.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(devices.len(), 3, "one terminal a block");
    // Each pair of symbols is one terminal, its levels numbered 1 and 2, 3 and 4, 5 and 6.
    for pair in symbols.chunks(2) {
        assert_eq!(pair[0].0, pair[1].0, "{symbols:?}");
    }
    let labels = cp_labels(&s);
    for n in 1..=6 {
        assert!(
            labels.iter().any(|l| l.ends_with(&format!(":{n}/a"))),
            "level number {n}: {labels:?}"
        );
    }
    // The block carries the starter's two-level part, once.
    done(&mut s, "select.set", json!({"ids": [symbols[0].1]}));
    let sheet = s.properties().sheet().expect("a sheet");
    let parts = match &find_row(sheet, "device/parts").expect("the Parts row").1.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    };
    assert_eq!(parts, "EW-GEN-TRM-3 × 1");
    done(&mut s, "edit.undo", json!({}));
    assert!(terminal_symbols(&s).is_empty(), "one Undo takes the row back");
}

#[test]
fn the_join_chooser_lists_free_terminals_the_nearest_first() {
    let tmp = Tmp::new("terminal-join-free");
    let mut s = new_project(&tmp);
    place_row(&mut s, json!({"strip": "new", "count": "5"}));
    let symbols = terminal_symbols(&s);
    let tag = |s: &ShellModel, rep: &str| {
        tables(s)
            .cps
            .iter()
            .find(|c| c.rep.to_string() == rep)
            .map(|c| c.label.split('/').next().unwrap_or_default().to_owned())
            .expect("a label")
    };
    let (one, two, three) = (
        tag(&s, &symbols[0].1),
        tag(&s, &symbols[1].1),
        tag(&s, &symbols[2].1),
    );
    let four = tag(&s, &symbols[3].1);
    // 1 joins 2: 2 is a block of two levels now.
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "select.set", json!({"ids": [symbols[0].1]}));
    done(&mut s, "place.join", json!({}));
    let row = s
        .chooser()
        .expect("the chooser")
        .rows
        .iter()
        .position(|r| r.name == two)
        .expect("2 is offered to 1");
    done(&mut s, "chooser.choose", json!({"row": row}));
    // 3's chooser: 2 is no longer free; 4 is as near as 2 was and comes first.
    done(&mut s, "select.set", json!({"ids": [symbols[2].1]}));
    done(&mut s, "place.join", json!({}));
    let names: Vec<String> = s
        .chooser()
        .expect("the chooser")
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect();
    assert!(!names.contains(&two), "a block is not offered: {names:?}");
    assert!(!names.contains(&one), "1 is a level of 2 now: {names:?}");
    assert_eq!(names.first(), Some(&four), "the nearest first: {names:?}");
    assert!(!names.contains(&three), "not itself: {names:?}");
}
