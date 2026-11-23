// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bridge terminals where a person looks (doc 08 §6.3): a single terminal's menu shows it greyed with how a bridge is
//! made; in the Terminal strips table a row of a block of two levels is that level and selects that level's symbol,
//! and F2 on the Bridge cell over marked rows bridges them at their level (a level-1 bridge kept the level-2 bridge
//! from being drawn), while one row says how a bridge is made. Three blocks of two levels on a new project's page.

use crate::common;

use common::{Tmp, done, failed, tables};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// A new project with a row of three blocks of two levels; its terminal symbols left to right as (rep, level).
fn blocks(tmp: &Tmp) -> (ShellModel, Vec<(String, u8)>) {
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
        "tool.place_terminal",
        json!({"strip": "new", "levels": "2", "count": "3"}),
    );
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(10), sn.page_of_index(20)))
    };
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    let t = tables(&s);
    let mut reps: Vec<_> = t.reps.iter().filter(|r| r.terminal).collect();
    reps.sort_by_key(|r| r.pos);
    let symbols = reps
        .iter()
        .map(|r| {
            let level = t
                .cps
                .iter()
                .find(|c| c.rep == r.id && c.level.is_some())
                .and_then(|c| c.level)
                .unwrap_or(1);
            (r.id.to_string(), level)
        })
        .collect();
    (s, symbols)
}

/// The Terminal strips table's rows as (key, Bridge cell, the records its row selects).
fn rows(s: &ShellModel) -> Vec<(String, String, Vec<String>)> {
    let t = s.nav(NavKind::Strips).and_then(|n| n.table()).expect("the table");
    let bridge = t.columns.iter().position(|c| c.key == "bridge").expect("Bridge");
    t.nodes
        .iter()
        .filter(|n| n.key.contains("terminal:"))
        .map(|n| {
            (
                n.key.clone(),
                n.cells[bridge].clone(),
                n.drawables.iter().map(|d| d.record.to_string()).collect(),
            )
        })
        .collect()
}

#[test]
fn a_single_terminals_menu_greys_bridge_terminals_with_how() {
    let tmp = Tmp::new("bridge-one-terminal");
    let (mut s, symbols) = blocks(&tmp);
    done(&mut s, "select.set", json!({"ids": [symbols[0].0]}));
    let menu = canvas_menu(&s, MenuSpot::Object, None);
    let bridge = menu
        .iter()
        .find(|i| i.id == "strip.bridge")
        .expect("Bridge terminals in a single terminal's menu");
    assert!(!bridge.enabled);
    assert_eq!(
        bridge.reason.as_deref(),
        Some("Select two or more terminals of one strip")
    );
}

#[test]
fn a_level_row_selects_its_symbol_and_f2_on_bridge_bridges_at_that_level() {
    let tmp = Tmp::new("bridge-levels");
    let (mut s, symbols) = blocks(&tmp);
    let of_level = |l: u8| -> Vec<String> {
        symbols
            .iter()
            .filter(|(_, x)| *x == l)
            .map(|(r, _)| r.clone())
            .collect()
    };
    let (one, two) = (of_level(1), of_level(2));
    assert_eq!((one.len(), two.len()), (3, 3), "{symbols:?}");
    // Level 1 of the first two blocks bridged on the page.
    done(&mut s, "select.set", json!({"ids": one[..2]}));
    done(&mut s, "strip.bridge", json!({}));
    done(&mut s, "select.set", json!({"ids": []}));
    done(&mut s, "navigator.show", json!({"kind": "strips"}));
    done(&mut s, "navigator.table", json!({"kind": "strips", "on": true}));
    // Each row selects one symbol, its level's; a level-2 row has a key of its own.
    let level_two: Vec<(String, String, Vec<String>)> = rows(&s)
        .into_iter()
        .filter(|(k, _, _)| k.ends_with(":2"))
        .collect();
    assert_eq!(level_two.len(), 3, "{:?}", rows(&s));
    for (_, _, records) in &level_two {
        assert!(records.len() == 1 && two.contains(&records[0]), "{records:?}");
    }
    assert!(
        level_two.iter().all(|(_, b, _)| b.is_empty()),
        "level 2 not bridged yet"
    );
    // One row: F2 on Bridge says how a bridge is made.
    let keys: Vec<String> = level_two[..2].iter().map(|(k, _, _)| k.clone()).collect();
    done(
        &mut s,
        "table.cell",
        json!({"kind": "strips", "row": keys[0], "column": "bridge"}),
    );
    let how = failed(&mut s, "table.edit", json!({"kind": "strips"}));
    assert!(how.contains("press F2 in this column"), "{how}");
    // Two level-2 rows marked: F2 bridges them on level 2.
    done(
        &mut s,
        "navigator.select",
        json!({"kind": "strips", "keys": keys}),
    );
    done(&mut s, "table.edit", json!({"kind": "strips"}));
    let bridged: Vec<String> = rows(&s)
        .into_iter()
        .filter(|(k, _, _)| keys.contains(k))
        .map(|(_, b, _)| b)
        .collect();
    assert!(
        bridged.iter().all(|b| !b.is_empty()),
        "bridged on level 2: {bridged:?}"
    );
}
