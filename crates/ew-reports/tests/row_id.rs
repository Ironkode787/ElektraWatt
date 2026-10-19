// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text form of `RowId`: typed, escaped, injective (doc 09 §2.3, FR-EXP-004, FR-REP-048).

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::Device;
use ew_reports::{RowId, RowKey};
use proptest::prelude::*;

const U: &str = "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37";

fn name(s: &str) -> RowKey {
    RowKey::Name(s.to_owned())
}

#[test]
fn an_entity_prints_as_its_typed_uuid() {
    let u: Uuid = U.parse().unwrap();
    let id: Id<Device> = Id::from_uuid(u);
    assert_eq!(RowId::entity(id).to_string(), format!("e:{U}"));
}

#[test]
fn components_are_typed_and_joined_by_slash() {
    let u: Uuid = U.parse().unwrap();
    let id = RowId(vec![RowKey::Entity(u), RowKey::Index(7), name("component")]);
    assert_eq!(id.to_string(), format!("e:{U}/i:7/n:component"));
    assert_eq!(id.to_string().parse::<RowId>(), Ok(id));
}

#[test]
fn the_empty_id_prints_as_the_empty_string() {
    assert_eq!(RowId(Vec::new()).to_string(), "");
    assert_eq!(RowId::parse(""), Ok(RowId(Vec::new())));
    // A single empty name is not the empty ID.
    assert_eq!(RowId(vec![name("")]).to_string(), "n:");
    assert_eq!(RowId::parse("n:"), Ok(RowId(vec![name("")])));
}

#[test]
fn a_slash_in_a_name_differs_from_two_components() {
    let one = RowId(vec![name("a/b")]);
    let two = RowId(vec![name("a"), name("b")]);
    assert_ne!(one.to_string(), two.to_string());
    assert_eq!(one.to_string(), "n:a%2Fb");
    assert_eq!(two.to_string(), "n:a/n:b");
}

#[test]
fn a_name_and_an_index_or_entity_never_print_alike() {
    assert_ne!(
        RowId(vec![name("7")]).to_string(),
        RowId(vec![RowKey::Index(7)]).to_string()
    );
    assert_ne!(
        RowId(vec![name(U)]).to_string(),
        RowId(vec![RowKey::Entity(U.parse().unwrap())]).to_string()
    );
}

#[test]
fn names_are_percent_escaped_per_utf8_byte_in_uppercase() {
    assert_eq!(RowId(vec![name("50%:x")]).to_string(), "n:50%25%3Ax");
    assert_eq!(RowId(vec![name("a b")]).to_string(), "n:a%20b");
    assert_eq!(RowId(vec![name("é")]).to_string(), "n:%C3%A9");
    assert_eq!(RowId(vec![name("Az09-_.~")]).to_string(), "n:Az09-_.~");
    for s in ["50%:x", "a b", "é", "日本", "a/b/", ""] {
        let id = RowId(vec![name(s)]);
        assert_eq!(RowId::parse(&id.to_string()), Ok(id));
    }
}

#[test]
fn escapes_are_read_in_either_case() {
    assert_eq!(RowId::parse("n:%c3%a9"), Ok(RowId(vec![name("é")])));
    assert_eq!(RowId::parse("n:%C3%a9"), Ok(RowId(vec![name("é")])));
}

#[test]
fn malformed_texts_are_rejected() {
    let bad = [
        "a",                                      // no tag
        "x:1",                                    // unknown tag
        "e:not-a-uuid",                           // bad uuid
        "e:7F0C2A91-5D3E-7B48-9A61-0E2F4C8D1B37", // uppercase uuid
        "e:7f0c2a915d3e7b489a610e2f4c8d1b37",     // not hyphenated
        "i:",                                     // empty number
        "i:-1",                                   // negative
        "i:+1",                                   // plus sign
        "i:007",                                  // leading zeros
        "i:4294967296",                           // overflows u32
        "n:a%2",                                  // truncated escape
        "n:a%zz",                                 // bad hex
        "n:%FF",                                  // invalid UTF-8
        "n:a b",                                  // raw space
        "n:a:b",                                  // raw colon
        "n:é",                                    // raw non-ASCII
        "n:a%",                                   // lone percent
        "n:a//n:b",                               // empty component
        "n:a/",                                   // trailing slash
        "/n:a",                                   // leading slash
    ];
    for s in bad {
        let err = RowId::parse(s).expect_err(s);
        assert!(!err.message().is_empty(), "{s}");
        assert!(err.to_string().starts_with("invalid row ID"), "{s}");
    }
}

fn key() -> impl Strategy<Value = RowKey> {
    prop_oneof![
        any::<u128>().prop_map(|n| RowKey::Entity(Uuid::from_u128(n))),
        any::<u32>().prop_map(RowKey::Index),
        any::<String>().prop_map(RowKey::Name),
        // Names heavy in the characters that need escaping.
        "[a-c/:%é 1-3]{0,6}".prop_map(RowKey::Name),
    ]
}

fn row_id() -> impl Strategy<Value = RowId> {
    prop::collection::vec(key(), 0..6).prop_map(RowId)
}

proptest! {
    #[test]
    fn display_then_parse_is_the_identity(id in row_id()) {
        prop_assert_eq!(RowId::parse(&id.to_string()), Ok(id));
    }

    #[test]
    fn different_ids_print_differently(a in row_id(), b in row_id()) {
        if a != b {
            prop_assert_ne!(a.to_string(), b.to_string());
        }
    }
}
