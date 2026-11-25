// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New page… asked for while a station's view is in front: its Connection grid comes filled with the grid of the page
//! tab beside the view, as it does from a page.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, ShellModel};
use serde_json::json;

/// The value of the New page form's field `name`.
fn field(s: &ShellModel, name: &str) -> String {
    match s.dialog() {
        Some(Dialog::Form(f)) => f
            .fields
            .iter()
            .find(|x| x.name == name)
            .map(|x| x.value.clone())
            .unwrap_or_default(),
        other => panic!("the New page form: {other:?}"),
    }
}

#[test]
fn new_page_from_a_station_s_view_comes_with_the_page_s_grid() {
    let tmp = Tmp::new("new-page-from-station");
    let mut s = tmp.empty_shell(Mode::Gui);
    let project = json!({"dir": tmp.0.join("p.ew").to_string_lossy(), "template": "builtin"});
    done(&mut s, "file.new", project);
    done(&mut s, "page.new", json!({}));
    let from_page = field(&s, "grid");
    assert!(!from_page.is_empty(), "from a page the grid is filled");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "plc.new_station", json!({}));
    assert!(s.canvas().is_none(), "the station's view is in front");
    done(&mut s, "page.new", json!({}));
    assert_eq!(
        field(&s, "grid"),
        from_page,
        "the same grid from the station's view"
    );
    s.shutdown();
}
