// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A terminal's two ends read apart on the page (doc 08 §6.3): its symbol looks the same turned half round, so R
//! seemed to do nothing and a person could not tell clamp a from clamp b. With a terminal selected, the overlay names
//! the clamp at each end, and a half turn moves the names with the ends.

use crate::common;

use common::{Tmp, done, tables};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::tools::{Anchor, OverlayItem};
use serde_json::json;

/// The clamp names the overlay shows, from the upper end to the lower (sheet y grows downwards), each centred on the
/// line of the terminal's lead, so a turned terminal's letters stay on its ends' line.
fn clamps(s: &ShellModel) -> Vec<String> {
    let mut v: Vec<((i64, i64), String, Anchor)> = s
        .tool_overlay()
        .items
        .iter()
        .filter_map(|i| match i {
            OverlayItem::Label { text, anchor, at } => Some((*at, text.clone(), *anchor)),
            _ => None,
        })
        .collect();
    v.sort_by_key(|l| l.0.1);
    for l in &v {
        assert_eq!((l.0.0, l.2), (v[0].0.0, Anchor::Centre), "{}", l.1);
    }
    v.into_iter().map(|(_, t, _)| t).collect()
}

#[test]
fn a_selected_terminal_names_its_clamps_and_a_half_turn_moves_them() {
    let tmp = Tmp::new("terminal-ends");
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(&mut s, "tool.place_terminal", json!({"strip": "new"}));
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(30), sn.page_of_index(20)))
    };
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    let rep = tables(&s)
        .reps
        .iter()
        .find(|r| r.terminal)
        .map(|r| r.id.to_string())
        .expect("the terminal");
    done(&mut s, "select.set", json!({"ids": [rep]}));
    // `ew.iec.terminal` draws clamp a (side 1) at its lower end and clamp b at its upper end.
    assert_eq!(clamps(&s), ["b", "a"]);
    done(&mut s, "tool.rotate_cw", json!({}));
    done(&mut s, "tool.rotate_cw", json!({}));
    assert_eq!(
        clamps(&s),
        ["a", "b"],
        "turned half round, clamp a is the upper end"
    );
    done(&mut s, "select.set", json!({"ids": []}));
    assert!(clamps(&s).is_empty(), "nothing selected, no names");
    // Selected by its tag, as a script selects it.
    let tag = tables(&s)
        .cps
        .iter()
        .find(|c| c.rep.to_string() == rep)
        .and_then(|c| c.label.split('/').next().map(str::to_owned))
        .expect("its tag");
    done(&mut s, "select.set", json!({"refs": [format!("tag:{tag}")]}));
    assert_eq!(clamps(&s), ["a", "b"]);
}

/// The letters show while at most four terminals are selected (twenty-four letters hid twelve symbols), and the hint
/// line says which clamp is the cabinet's side (doc 08 §2.1: side 1 internal, side 2 the field).
#[test]
fn the_letters_show_for_a_few_terminals_and_the_hint_names_the_cabinet_side() {
    let tmp = Tmp::new("terminal-ends-many");
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
        json!({"strip": "new", "count": "5"}),
    );
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(30), sn.page_of_index(20)))
    };
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    let reps: Vec<String> = tables(&s)
        .reps
        .iter()
        .filter(|r| r.terminal)
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(reps.len(), 5);
    let letters = |s: &ShellModel| {
        s.tool_overlay()
            .items
            .iter()
            .filter(|i| matches!(i, OverlayItem::Label { .. }))
            .count()
    };
    done(&mut s, "select.set", json!({"ids": reps[..4]}));
    assert_eq!(letters(&s), 8, "four terminals, two letters each");
    done(&mut s, "select.set", json!({ "ids": reps }));
    assert_eq!(letters(&s), 0, "five terminals: no letters");
    let prompt = ew_viewmodel::command_bar::hud(&s).prompt;
    assert!(
        prompt.ends_with("clamp a is the internal side (the cabinet's wiring), clamp b the field side"),
        "{prompt}"
    );
}
