// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A new project's supply is set in its Properties (doc 06 §3.1): its Supply rows stand while it has none, the
//! first value set makes it, and a value no supply has is refused in words.

use crate::common::{Tmp, done, new_project};
use ew_viewmodel::properties::{RowValue, value_text};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The text of the project's row `key`.
fn shown(s: &ShellModel, key: &str) -> Option<String> {
    let sheet = s.properties().sheet()?;
    let row = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .find(|r| r.key == key)?;
    match &row.value {
        RowValue::Absent => Some(String::new()),
        _ => Some(value_text(row)),
    }
}

/// Sets the project's row `key` to `text`, through the card a bulk edit opens.
fn set(s: &mut ShellModel, key: &str, text: &str) -> ew_viewmodel::shell::Outcome {
    let out = s.dispatch("props.set", &json!({"row": key, "value": text}));
    if s.properties().card().is_some_and(|c| c.preview.result.is_ok()) {
        return s.dispatch("preview.apply", &json!({}));
    }
    out
}

#[test]
fn a_new_projects_supply_is_set_in_its_properties() {
    let tmp = Tmp::new("project-supply-rows");
    let mut s = new_project(&tmp);
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.show_advanced", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    assert_eq!(
        shown(&s, "project/supply.phases").as_deref(),
        Some(""),
        "the row stands, empty"
    );
    assert_eq!(
        set(&mut s, "project/supply.phases", "3"),
        ew_viewmodel::shell::Outcome::Done
    );
    assert_eq!(shown(&s, "project/supply.phases").as_deref(), Some("3"));
    assert_eq!(
        shown(&s, "project/supply.u_nom").as_deref(),
        Some("230"),
        "the new supply's voltage"
    );
    // Four phases is no supply: refused in words, the row as it was.
    let out = set(&mut s, "project/supply.phases", "4");
    assert_ne!(out, ew_viewmodel::shell::Outcome::Done);
    assert_eq!(shown(&s, "project/supply.phases").as_deref(), Some("3"));
}
