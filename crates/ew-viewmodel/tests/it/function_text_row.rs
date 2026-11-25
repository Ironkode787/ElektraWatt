// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A device's Function text row says where the text prints and offers the way to print it:
//! its help names the place and Function
//! text over its path, and its … menu runs that. The sample's page 102, its contactor -102Q1.

use crate::common;

use common::{Tmp, actual_size, done, select_ids, tables};
use ew_viewmodel::properties::RowMenuItem;
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

const ROW: &str = "device/function_text";

/// Selects the symbol of the device printed `tag` on the page shown.
fn select_symbol(s: &mut ShellModel, tag: &str) {
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
}

#[test]
fn the_function_text_row_says_where_it_prints_and_prints_it_over_the_path() {
    assert!(
        ew_viewmodel::strings::field_help("device", "function_text")
            .is_some_and(|h| h.contains("Function text over its path")),
        "the row's help names the way to print it"
    );
    let tmp = Tmp::new("function-text-row");
    let mut s = actual_size(&tmp, "102");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "select.set", json!({"refs": ["tag:-102Q1"]}));
    done(&mut s, "props.set", json!({"row": ROW, "value": "Conveyor run"}));
    select_symbol(&mut s, "-102Q1");
    let menu = s.row_menu(ROW);
    let at = menu
        .iter()
        .position(|i| *i == RowMenuItem::FunctionOverPath)
        .unwrap_or_else(|| panic!("the row's menu offers it: {menu:?}"));
    done(&mut s, "props.row", json!({"to": ROW}));
    done(&mut s, "props.row_menu", json!({}));
    done(&mut s, "props.choose", json!({"index": at}));
    assert!(
        tables(&s)
            .items
            .iter()
            .any(|i| i.bind.as_deref() == Some("device.function_text")),
        "the text stands on the page"
    );
}
