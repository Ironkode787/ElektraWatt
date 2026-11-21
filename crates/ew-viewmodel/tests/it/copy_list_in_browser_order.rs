// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1810: the symbol editor's Copy a library symbol was one long list without order or preview. Its symbols come in
//! the symbol browser's order, by the words of the function category each is filed under (Other last), and each has
//! the shape the browser previews.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use ew_viewmodel::tools::category_words;
use serde_json::json;

#[test]
fn the_copy_list_is_in_the_browsers_order_with_a_preview() {
    let tmp = Tmp::new("copy-order");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "symbol.new", json!({}));
    let editor = s.symbol_editor().expect("the editor");
    let keys: Vec<(bool, String)> = editor
        .sources
        .iter()
        .map(|src| {
            let filed = category_words(&src.category);
            (filed.is_none(), filed.unwrap_or_default().to_lowercase())
        })
        .collect();
    assert!(keys.len() > 2);
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "{keys:?}");
    assert!(!keys[0].0, "a symbol filed under a category first");
    let first = &editor.sources[0];
    assert!(s.symbol_source_shape(first).is_some(), "{}", first.source);
}
