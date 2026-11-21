// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Texts of a symbol put on the page from the canvas menu (FT-1875; doc 06 §3.8), on the sample's page 102: Function
//! text over its path over one symbol of a device with a function text, refused in words without one, and Name the
//! terminal strip here over one terminal.

use crate::common;

use common::{Tmp, actual_size, done, refused, select_ids, tables};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// Selects the symbol of the device printed `tag` on the page shown; its ID.
fn select_symbol(s: &mut ShellModel, tag: &str) -> String {
    done(s, "select.set", json!({"refs": [format!("tag:{tag}")]}));
    let device = s.selection().items[0].0;
    let rep = tables(s)
        .reps
        .iter()
        .find(|r| r.device == Some(device))
        .expect("its symbol")
        .id
        .to_string();
    select_ids(s, std::slice::from_ref(&rep));
    rep
}

/// The `free_text` of the last commit, an `item.place`.
fn placed(s: &ShellModel) -> serde_json::Value {
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    last.params["kind"]["free_text"].clone()
}

#[test]
fn a_function_text_over_its_path_and_a_strips_name_are_placed_from_the_canvas_menu() {
    let tmp = Tmp::new("symbol-texts");
    let mut s = actual_size(&tmp, "102");
    // A device without a function text: refused, as it would print nothing.
    select_symbol(&mut s, "-102Q1");
    assert_eq!(
        refused(&mut s, "text.function_over_path", json!({})),
        "reason-no-function-text"
    );
    done(&mut s, "select.set", json!({"refs": ["tag:-102Q1"]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/function_text", "value": "Conveyor run"}),
    );
    let coil = select_symbol(&mut s, "-102Q1");
    done(&mut s, "text.function_over_path", json!({}));
    let free = placed(&s);
    assert_eq!(
        (free["bind"].as_str(), free["of"].as_str()),
        (Some("device.function_text"), Some(coil.as_str()))
    );
    let body = tables(&s)
        .reps
        .iter()
        .find(|r| r.id.to_string() == coil)
        .unwrap()
        .body;
    assert!(
        free["at"][1].as_i64().unwrap() < body[1],
        "over the symbol: {free}"
    );
    assert!(
        tables(&s)
            .items
            .iter()
            .any(|i| i.bind.as_deref() == Some("device.function_text"))
    );
    // A terminal names its strip.
    let terminal = tables(&s)
        .reps
        .iter()
        .find(|r| r.terminal)
        .expect("a terminal")
        .id
        .to_string();
    select_ids(&mut s, std::slice::from_ref(&terminal));
    done(&mut s, "text.strip_name", json!({}));
    let free = placed(&s);
    assert_eq!(
        (free["bind"].as_str(), free["of"].as_str()),
        (Some("strip.tag"), Some(terminal.as_str()))
    );
}
