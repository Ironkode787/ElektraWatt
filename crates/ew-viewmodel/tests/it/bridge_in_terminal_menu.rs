// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bridge terminals is in the menu of selected terminals (a persona run selected two terminals and looked there), and
//! the command search finds it under "bridge". Two terminals of one strip on the sample's 103.

use crate::common;

use common::{Tmp, done, tables};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn two_selected_terminals_offer_bridge_terminals() {
    let tmp = Tmp::new("bridge-in-terminal-menu");
    let mut s = tmp.sample_in(Mode::Headless, true, "103");
    let t = tables(&s);
    let strip = |label: &str| label.split(':').next().unwrap_or_default().to_owned();
    // Two terminals whose points name one strip.
    let terminals: Vec<(String, String)> = t
        .reps
        .iter()
        .filter(|r| r.terminal)
        .filter_map(|r| {
            let cp = t.cps.iter().find(|c| c.rep == r.id)?;
            Some((r.id.to_string(), strip(&cp.label)))
        })
        .collect();
    let (first, rest) = terminals.split_first().expect("a terminal");
    let second = rest
        .iter()
        .find(|(_, st)| *st == first.1)
        .expect("a second terminal of the strip");
    done(&mut s, "select.set", json!({"ids": [first.0, second.0]}));
    let menu = canvas_menu(&s, MenuSpot::Object, None);
    let bridge = menu
        .iter()
        .find(|i| i.id == "strip.bridge")
        .expect("Bridge terminals in the menu");
    assert!(bridge.enabled, "{:?}", bridge.reason);
    done(&mut s, "tools.palette", json!({}));
    s.set_palette_query("bridge");
    assert!(s.palette_entries().iter().any(|e| e.id == "strip.bridge"));
}
