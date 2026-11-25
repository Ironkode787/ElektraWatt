// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a wire's number is where a person reads it (FT-1858, FT-1799; doc 08 §5.2, §5.3), on the sample: a frozen
//! number in Properties' title and Number row, in the SQL view and in the wire list; the number typed in its row; and
//! a wire the per-potential scheme gives no code titled in words.

use ew_api::Session;
use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::sql::Profile;
use ew_derive::WireNumbers;
use ew_ids::Uuid;
use ew_mltext::Ident;
use ew_schema::{NumberSpec, Record};
use serde_json::{Value, json};

/// The carrier of the sample's wire printed `number`.
fn wire(s: &Session, number: &str) -> Uuid {
    s.query()
        .get::<WireNumbers>(())
        .wires
        .iter()
        .find(|w| w.number.as_ref().is_some_and(|n| n.label == number))
        .map(|w| w.wire.as_uuid())
        .unwrap_or_else(|| panic!("no wire {number}"))
}

/// The sample with `edit` applied to its records before the session is built.
fn sample_with(edit: impl FnOnce(&mut ew_schema::Project, &mut Vec<Record>)) -> Session {
    let (mut project, mut records, lib) = crate::common::sample_records();
    edit(&mut project, &mut records);
    crate::common::session_of(project, records, lib)
}

/// The one value of the Number row of `wire`'s sheet.
fn number_row(s: &Session, wire: Uuid) -> String {
    let sheet = s.property_sheet(&[wire]);
    let row = sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .find(|r| r.path == "number")
        .expect("a Number row");
    match &row.value {
        ew_api::output::RowValue::One { text, .. } => text.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_frozen_number_prints_in_properties_the_sql_view_and_the_wire_list() {
    let w = wire(&crate::common::sample(), "102.1");
    let s = sample_with(|_, records| {
        for r in records.iter_mut() {
            if let Record::Connection(c) = r
                && c.id.as_uuid() == w
            {
                c.number = NumberSpec::Frozen(Ident::from_input("6A03").unwrap());
            }
        }
    });
    assert_eq!(s.property_sheet(&[w]).header.title, "6A03");
    assert_eq!(number_row(&s, w), "6A03");
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let p = s.sql_projection(Some(&report)).expect("the projection builds");
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let rows = reader
        .query(&format!("SELECT number FROM api_v1_wire WHERE id = '{w}'"))
        .unwrap();
    assert_eq!(rows.rows[0][0].text(), "6A03");
    let list = ew_api::output::row_export(&s.query(), &ew_api::output::RowExport::Wires, None)
        .expect("the wire list derives");
    assert!(
        list.csv.lines().any(|l| l.split(',').any(|cell| cell == "6A03")),
        "the wire list prints the frozen number: {}",
        list.csv
    );
}

#[test]
fn the_number_row_takes_a_typed_number() {
    let s = crate::common::sample();
    let value = |text: &str| s.parse_field_input("connection", "number", text).unwrap().value;
    assert_eq!(value("6A03"), json!({"frozen": "6A03"}));
    assert_eq!(value(" \"6A03\" "), json!({"frozen": "6A03"}));
    assert_eq!(value(""), Value::Null, "empty unfreezes");
    assert_eq!(value(r#"{"frozen": "7A03"}"#), json!({"frozen": "7A03"}));
    assert!(s.parse_field_input("connection", "number", "6\tA03").is_err());
}

#[test]
fn a_wire_without_a_number_is_titled_in_words() {
    let s = sample_with(|project, _| {
        project
            .settings
            .extra
            .insert("drawing".to_owned(), json!({"wire_numbering": "per_potential"}));
    });
    let numbers = s.query().get::<WireNumbers>(());
    let bare = numbers
        .wires
        .iter()
        .find(|w| {
            w.number.is_none()
                && s.query()
                    .connections()
                    .get(w.wire)
                    .is_some_and(|c| c.page.is_some() && c.kind == ew_schema::ConnectionKind::Wire)
        })
        .expect("a drawn wire whose potential the sample does not name");
    assert_eq!(
        s.property_sheet(&[bare.wire.as_uuid()]).header.title,
        ew_api::output::UNNUMBERED_WIRE
    );
}
