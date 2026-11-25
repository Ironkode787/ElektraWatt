// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The words of the PLC exchange: every key `ew-plc` names an object, a field or a merge state by is a text
//! of the window's files, so a notice never prints `?` and the window renders each in its language.

use ew_commands::text_english;
use ew_plc::{FIELD_KEYS, State, object_key};
use ew_schema::ExchangeObject;

#[test]
fn every_key_of_the_exchange_words_is_a_text() {
    let objects = [
        ExchangeObject::Station,
        ExchangeObject::Module,
        ExchangeObject::Channel,
        ExchangeObject::Tag,
        ExchangeObject::Node,
        ExchangeObject::Instrument,
        ExchangeObject::Unknown(serde_json::Value::from("newer")),
    ];
    let states = [
        State::Apply,
        State::Rejected,
        State::Kept,
        State::Pending,
        State::Conflict,
    ];
    let keys = objects
        .iter()
        .map(object_key)
        .chain(FIELD_KEYS.iter().map(|(_, _, key)| *key))
        .chain(states.into_iter().map(State::key));
    for key in keys {
        assert!(text_english(key).is_some(), "{key}");
    }
}
