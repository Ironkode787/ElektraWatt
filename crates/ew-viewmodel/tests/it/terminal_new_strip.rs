// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A new terminal strip is a new strip (rc.2 persona run, entry 18): Terminal… on a page that has no terminal yet takes
//! a new strip, with the tag given; on a page with a terminal it goes on with the strip there.

use crate::common;

use common::{Tmp, at, done, new_project, toast};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// Places one terminal by Terminal… with `params` at the grid index `k`; the placement's toast.
fn place_terminal(s: &mut ShellModel, params: serde_json::Value, k: (i64, i64)) -> String {
    done(s, "tool.place_terminal", params);
    let page = std::sync::Arc::clone(s.canvas().unwrap().page());
    let p = (page.snapper.page_of_index(k.0), page.snapper.page_of_index(k.1));
    done(s, "cursor.set", at(page.snapper.page_to_sheet(p), json!({})));
    done(s, "canvas.activate", json!({}));
    done(s, "tool.select", json!({}));
    toast(s)
}

#[test]
fn a_page_without_a_terminal_takes_a_new_strip_with_the_tag_given() {
    let tmp = Tmp::new("terminal-new-strip");
    let mut s = new_project(&tmp);
    done(&mut s, "view.zoom_actual", json!({}));
    let first = place_terminal(&mut s, json!({"count": 1, "tag": "-X1"}), (40, 30));
    assert!(first.starts_with("Placed -X1 terminal 1"), "{first}");
    // The same page goes on with its strip.
    let next = place_terminal(&mut s, json!({"count": 1}), (50, 30));
    assert!(next.starts_with("Placed -X1 terminal 2"), "{next}");
    // Another page without a terminal: a new strip, not -X1's terminal 3.
    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "multi_line", "title": "Field"}),
    );
    done(&mut s, "view.zoom_actual", json!({}));
    let field = place_terminal(&mut s, json!({"count": 1, "tag": "-X2"}), (40, 30));
    assert!(field.starts_with("Placed -X2 terminal 1"), "{field}");
}
