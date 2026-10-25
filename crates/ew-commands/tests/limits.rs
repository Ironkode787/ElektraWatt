// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The size limits of untyped command parameters (finding F6): a document exactly at a bound is accepted,
//! one over is rejected with the JSON Pointer of the offending value, an oversize document is rejected
//! fast, and messages never quote more than a short excerpt of the instance.

// A wall-clock timer of this test, asserted only with EW_PERF_ASSERT (the timing rule of doc 18 §3.16); no timestamp reaches a compared result.
#![allow(clippy::disallowed_methods)]

use std::time::{Duration, Instant};

use ew_commands::{
    AnyCommand, CmdError, CommandRegistry, MAX_PARAM_BYTES, MAX_PARAM_DEPTH, MAX_PARAM_ITEMS,
    MAX_PARAM_STRING, SchemaViolation, check_limits, validate_schema,
};
use serde_json::{Value, json};

fn rejected(doc: &Value) -> SchemaViolation {
    check_limits(doc).expect_err("must be rejected")
}

/// `depth` arrays nested in each other around `0`: `[[0]]` is depth 2.
fn nested(depth: usize) -> Value {
    let mut v = json!(0);
    for _ in 0..depth {
        v = Value::Array(vec![v]);
    }
    v
}

/// Four strings whose compact JSON length is `MAX_PARAM_BYTES` plus `extra` bytes.
fn document_of_length(extra: usize) -> Value {
    // `["…","…","…","…"]`: two brackets, three commas, four times two quotes.
    let overhead = 2 + 3 + 4 * 2;
    let payload = MAX_PARAM_BYTES + extra - overhead;
    let each = MAX_PARAM_STRING.min(payload / 4);
    let mut lengths = [each; 4];
    let mut rest = payload - 4 * each;
    for len in &mut lengths {
        let add = rest.min(MAX_PARAM_STRING - *len);
        *len += add;
        rest -= add;
    }
    assert_eq!(rest, 0, "the document must fit four strings");
    Value::Array(lengths.iter().map(|n| Value::String("x".repeat(*n))).collect())
}

#[test]
fn byte_length_bound() {
    let at = document_of_length(0);
    assert_eq!(
        serde_json::to_string(&at).expect("serialises").len(),
        MAX_PARAM_BYTES
    );
    assert_eq!(check_limits(&at), Ok(()));

    let over = document_of_length(1);
    assert_eq!(
        serde_json::to_string(&over).expect("serialises").len(),
        MAX_PARAM_BYTES + 1
    );
    let v = rejected(&over);
    assert_eq!(v.path, "");
    assert!(v.message.contains(&MAX_PARAM_BYTES.to_string()), "{}", v.message);
}

#[test]
fn nesting_bound() {
    assert_eq!(check_limits(&json!(0)), Ok(()));
    assert_eq!(check_limits(&nested(MAX_PARAM_DEPTH)), Ok(()));
    let over = nested(MAX_PARAM_DEPTH + 1);
    let v = rejected(&over);
    assert_eq!(v.path, "/0".repeat(MAX_PARAM_DEPTH));
    assert!(v.message.contains(&MAX_PARAM_DEPTH.to_string()), "{}", v.message);
    // Objects count as well.
    let mut obj = json!({});
    for _ in 1..MAX_PARAM_DEPTH {
        obj = json!({ "k": obj });
    }
    assert_eq!(check_limits(&obj), Ok(()));
    let v = rejected(&json!({ "k": obj }));
    assert_eq!(v.path, "/k".repeat(MAX_PARAM_DEPTH));
}

#[test]
fn a_very_deep_document_is_rejected_without_overflowing_the_stack() {
    let deep = nested(MAX_PARAM_DEPTH + 1);
    let v = rejected(&json!({ "a": [deep] }));
    assert!(v.message.contains(&MAX_PARAM_DEPTH.to_string()), "{}", v.message);
}

#[test]
fn array_item_bound() {
    let at = Value::Array(vec![json!(0); MAX_PARAM_ITEMS]);
    assert_eq!(check_limits(&json!({ "a": at })), Ok(()));
    let over = Value::Array(vec![json!(0); MAX_PARAM_ITEMS + 1]);
    let v = rejected(&json!({ "a": over }));
    assert_eq!(v.path, "/a");
    assert!(v.message.contains(&MAX_PARAM_ITEMS.to_string()), "{}", v.message);
}

#[test]
fn string_bound() {
    let at = "x".repeat(MAX_PARAM_STRING);
    assert_eq!(check_limits(&json!({ "s": at })), Ok(()));
    let v = rejected(&json!({ "s": "x".repeat(MAX_PARAM_STRING + 1) }));
    assert_eq!(v.path, "/s");
    assert!(v.message.contains(&MAX_PARAM_STRING.to_string()), "{}", v.message);
    // The bound counts UTF-8 bytes, not characters.
    let wide = "é".repeat(MAX_PARAM_STRING / 2 + 1);
    assert_eq!(rejected(&json!([[wide]])).path, "/0/0");
}

#[test]
fn object_keys_count_as_strings() {
    let mut at = serde_json::Map::new();
    at.insert("k".repeat(MAX_PARAM_STRING), json!(1));
    assert_eq!(check_limits(&Value::Object(at)), Ok(()));
    let mut over = serde_json::Map::new();
    over.insert("k".repeat(MAX_PARAM_STRING + 1), json!(1));
    let v = rejected(&Value::Object(over));
    assert!(v.message.contains(&MAX_PARAM_STRING.to_string()), "{}", v.message);
}

#[test]
fn ten_megabyte_string_fails_fast() {
    let cmd = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [], "path": "function_text",
            "value": "x".repeat(10 * 1024 * 1024)}),
    };
    // Timing acquisition and the generous bound are opt-in.
    let started = std::env::var_os("EW_PERF_ASSERT").is_some().then(Instant::now);
    let result = CommandRegistry::builtin().resolve(&cmd);
    match result {
        Err(CmdError::InvalidParams {
            command,
            path,
            message,
        }) => {
            assert_eq!(command, "fields.set");
            assert_eq!(path, "/value");
            assert!(message.contains("10485760"), "{message}");
        }
        other => panic!("expected InvalidParams, got {:?}", other.map(|c| c.name())),
    }
    if let Some(started) = started {
        let took = started.elapsed();
        let budget = if cfg!(debug_assertions) {
            Duration::from_secs(2)
        } else {
            Duration::from_millis(50)
        };
        assert!(took < budget, "resolve took {took:?}");
        eprintln!("ten_megabyte_string_fails_fast: resolve took {took:?}");
    }
}

#[test]
fn messages_render_at_most_256_chars_of_the_instance() {
    let big = "z".repeat(100_000);
    for schema in [
        json!({"const": "a"}),
        json!({"enum": ["a", "b"]}),
        json!({"oneOf": [{"const": "a"}, {"const": "b"}]}),
    ] {
        let v = validate_schema(&schema, &json!(big)).expect_err("must reject");
        assert!(v.message.chars().count() < 400, "{schema}: {}", v.message.len());
    }
    // A long const is clipped as well.
    let v = validate_schema(&json!({"const": big}), &json!(big.clone() + "!")).expect_err("rejected");
    assert!(v.message.chars().count() < 600, "{}", v.message.len());
    // Many long options: the list is clipped.
    let options: Vec<Value> = (0..1000).map(|i| json!(format!("option-{i}"))).collect();
    let v = validate_schema(&json!({"enum": options}), &json!("none")).expect_err("rejected");
    assert!(v.message.chars().count() < 400, "{}", v.message.len());
    // Through the registry: a 100 kB string where an enum is expected.
    let cmd = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": big, "targets": [], "path": "function_text", "value": "v"}),
    };
    let Err(CmdError::InvalidParams { message, .. }) = CommandRegistry::builtin().resolve(&cmd) else {
        panic!("must be rejected");
    };
    assert!(message.chars().count() < 400, "{}", message.len());
}

#[test]
fn unknown_name_wins_over_limits() {
    let cmd = AnyCommand {
        name: "no.such_command".into(),
        params: json!({ "value": "x".repeat(10 * 1024 * 1024) }),
    };
    assert!(matches!(
        CommandRegistry::builtin().resolve(&cmd),
        Err(CmdError::UnknownCommand { name }) if name == "no.such_command"
    ));
}
