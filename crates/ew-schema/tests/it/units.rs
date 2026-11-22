// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FT-1981: `Unit::ALL`, the one list the unit parsers read, holds every unit the schema names, and each unit is
//! named again by its spelling and by its symbol.

use std::collections::BTreeSet;

use ew_schema::Unit;
use serde_json::Value;

/// Every string an `enum` or a `const` of `schema` names.
fn named(schema: &Value, out: &mut BTreeSet<String>) {
    match schema {
        Value::Object(m) => {
            for (k, v) in m {
                match (k.as_str(), v) {
                    ("enum", Value::Array(items)) => {
                        out.extend(items.iter().filter_map(Value::as_str).map(str::to_owned));
                    }
                    ("const", Value::String(s)) => {
                        out.insert(s.clone());
                    }
                    _ => named(v, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| named(v, out)),
        _ => {}
    }
}

#[test]
fn every_unit_of_the_schema_is_in_the_list_and_named_back() {
    let schema = serde_json::to_value(schemars::schema_for!(Unit)).expect("the schema");
    let mut in_schema = BTreeSet::new();
    named(&schema, &mut in_schema);
    let listed: BTreeSet<String> = Unit::ALL.iter().filter_map(Unit::spelling).collect();
    assert_eq!(listed, in_schema);
    for u in Unit::ALL {
        let spelling = u.spelling().expect("a spelling");
        assert_eq!(Unit::named(&spelling), Some(u.clone()), "{spelling}");
        assert_eq!(Unit::named(u.symbol()), Some(u.clone()), "{}", u.symbol());
    }
}
