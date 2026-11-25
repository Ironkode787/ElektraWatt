// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A card drawn in cells prints in a channel's cell the channel's own text, its symbol name, where nothing is
//! wired to it yet, so the page says what an input is for before its device is drawn (doc 10 §3.6). A persona run gave
//! the symbol names in the station view and saw empty cells.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The texts the page on show prints.
fn texts(shell: &ShellModel) -> Vec<String> {
    use ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(group) => walk(&group.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    let list = std::sync::Arc::clone(&shell.canvas().expect("a page").page().list);
    walk(&list.items, &mut out);
    out
}

#[test]
fn a_channel_nothing_is_wired_to_prints_its_symbol_name_in_its_cell() {
    let tmp = Tmp::new("card-cell-symbol");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    done(&mut s, "plc.rack_editor", json!({"station": "tag:-104A1"}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    // A channel of the controller's first card that nothing is wired to.
    let spare = s
        .station_shown()
        .expect("the view")
        .channels
        .iter()
        .find(|c| c.device.is_none() && c.function_text.is_empty())
        .map(|c| c.channel.to_string())
        .expect("a channel nothing is wired to");
    done(
        &mut s,
        "plc.rack_channel",
        json!({"channel": spare, "column": "symbol"}),
    );
    done(&mut s, "plc.rack_channel", json!({"text": "Door_closed"}));
    done(&mut s, "plc.module_goto", json!({}));
    let printed = texts(&s);
    assert!(
        printed.iter().any(|t| t == "Door_closed"),
        "the card's cell prints the symbol name: {printed:?}"
    );
    s.shutdown();
}
