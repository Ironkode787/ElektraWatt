// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A field of the project's own typed in the window (doc 09 §2.10): the value of the window's language is set and a
//! value of several languages keeps the others, as a title block cell's text does.

use ew_mltext::MlText;
use serde_json::json;

fn value(s: &ew_api::Session, key: &str) -> Option<MlText> {
    let q = s.query();
    q.project()
        .user_props
        .iter()
        .find(|v| v.key.as_str() == key)
        .map(|v| v.value.clone())
}

#[test]
fn a_typed_value_sets_the_windows_language_and_keeps_the_others() {
    let mut s = crate::common::sample();
    crate::common::run_cmd(
        &mut s,
        "user_prop.define",
        json!({"label": "Order", "scope": "project", "value": {"en-GB": "Order 1", "nl-NL": "Bestelling 1"}}),
    );
    crate::common::run_cmd(
        &mut s,
        "user_prop.define",
        json!({"label": "Sheet", "scope": "project", "value": "A"}),
    );
    let set = |s: &mut ew_api::Session, key: &str, text: &str| {
        let edits = s.own_field_edits(false, &[], key, text, Some("nl-NL"));
        crate::common::run_cmd(s, "fields.set_many", json!({ "edits": edits }));
    };
    set(&mut s, "order", "Bestelling 2");
    let lang = |t: &str| t.parse::<ew_mltext::LangTag>().expect("a tag");
    let order = value(&s, "order").expect("a value");
    assert_eq!(
        order.get(&lang("en-GB")),
        Some("Order 1"),
        "the other language stays"
    );
    assert_eq!(order.get(&lang("nl-NL")), Some("Bestelling 2"));
    // A neutral value stays neutral; an emptied language goes, the other staying.
    set(&mut s, "sheet", "B");
    assert_eq!(value(&s, "sheet"), Some(MlText::neutral("B")));
    set(&mut s, "order", "");
    let order = value(&s, "order").expect("the English value stays");
    assert_eq!(order.get(&lang("nl-NL")), None);
    assert_eq!(order.get(&lang("en-GB")), Some("Order 1"));
}
