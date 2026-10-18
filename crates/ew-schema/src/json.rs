// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared serde helpers: the `extra` map, the number rule for unknown content, default predicates and the
//! [`Open`] wrapper that gives a foreign enum its `Unknown` catch-all.

use std::collections::BTreeMap;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::SchemaError;

/// The `extra` map of a record or value object: unknown members of a newer format minor and namespaced
/// `plugin.<id>.*` keys (FR-EXT-021). It is flattened last and written in byte-wise key order (doc 05 §3.1).
///
/// A writer never puts a known member name of the same object into it.
pub type Extra = BTreeMap<String, Value>;

/// The largest integer magnitude a persisted integer may have (doc 05 §3.2): 2⁵³ − 1.
pub use ew_units::MAX_SAFE_INTEGER;

/// Checks the number rule of doc 05 §3.2 on unknown content: no floats anywhere and every integer within
/// ±(2⁵³ − 1). Typed members enforce the rule through their own types; this function covers `extra`
/// values and the `Unknown` variants of persisted enums.
///
/// ```
/// use serde_json::json;
///
/// assert!(ew_schema::check_value(&json!({"a": [1, "x", {"b": -2}]})).is_ok());
/// assert!(ew_schema::check_value(&json!({"a": 1.5})).is_err());
/// assert!(ew_schema::check_value(&json!(9007199254740992_u64)).is_err());
/// ```
pub fn check_value(value: &Value) -> Result<(), SchemaError> {
    match value {
        Value::Number(n) => {
            let ok = match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&i),
                (None, Some(u)) => u <= MAX_SAFE_INTEGER as u64,
                (None, None) => false,
            };
            if ok {
                Ok(())
            } else if n.is_f64() {
                Err(SchemaError::Float)
            } else {
                Err(SchemaError::IntegerOutOfRange)
            }
        }
        Value::Array(items) => items.iter().try_for_each(check_value),
        Value::Object(map) => map.values().try_for_each(check_value),
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
    }
}

/// `deserialize_with` for flattened `extra` maps: reads the remaining members and applies
/// [`check_value`].
pub(crate) fn de_extra<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Extra, D::Error> {
    let map = Extra::deserialize(deserializer)?;
    for (key, value) in &map {
        check_value(value).map_err(|e| D::Error::custom(format!("member \"{key}\": {e}")))?;
    }
    Ok(map)
}

/// `deserialize_with` for the `Unknown(Value)` catch-all variants: reads any JSON value and applies
/// [`check_value`].
pub(crate) fn de_unknown<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    let value = Value::deserialize(deserializer)?;
    check_value(&value).map_err(D::Error::custom)?;
    Ok(value)
}

/// `skip_serializing_if` predicate: the value equals its type's default.
pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// `skip_serializing_if` predicate for `bool` members whose default is `false`.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

/// `skip_serializing_if` predicate for `bool` members whose default is `true`.
pub(crate) fn is_true(value: &bool) -> bool {
    *value
}

/// `default` for `bool` members whose default is `true`.
pub(crate) fn r#true() -> bool {
    true
}

/// `default` of the per-kind schema version `$v`.
pub(crate) fn v1() -> u16 {
    1
}

/// `skip_serializing_if` predicate of `$v`: omitted when 1 (doc 05 §3.1).
pub(crate) fn is_v1(v: &u16) -> bool {
    *v == 1
}

/// A persisted value of a foreign enum (one defined in a foundation crate without a catch-all), extended
/// with the `Unknown` variant every persisted enum needs (ADR-0011 amendment 1): a value written by a newer
/// minor stays readable and is written back unchanged.
///
/// ```
/// use ew_rds::Aspect;
/// use ew_schema::Open;
///
/// let known: Open<Aspect> = serde_json::from_str("\"location\"").unwrap();
/// assert_eq!(known, Open::Known(Aspect::Location));
/// let unknown: Open<Aspect> = serde_json::from_str("\"zone\"").unwrap();
/// assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"zone\"");
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Open<T> {
    /// A value this reader understands.
    Known(T),
    /// A value of a newer format, kept verbatim.
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

impl<T> Open<T> {
    /// The known value, if any.
    pub fn known(&self) -> Option<&T> {
        match self {
            Open::Known(t) => Some(t),
            Open::Unknown(_) => None,
        }
    }
}

impl<T> From<T> for Open<T> {
    fn from(value: T) -> Open<T> {
        Open::Known(value)
    }
}
