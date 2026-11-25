// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rows of the internal tables, filled by column name, and the value conventions of doc 17 §6.1.

use ew_mltext::MlText;
use ew_schema::{ColourCode, ColourSpec, Csa};
use rusqlite::types::Value as SqlValue;
use serde::Serialize;

use crate::schema::{ColType, View};

/// One row of a view's table; columns that are not set are `NULL`.
pub(crate) struct Row {
    view: &'static View,
    values: Vec<SqlValue>,
}

impl Row {
    /// An empty row of `view`.
    pub fn new(view: &'static View) -> Row {
        Row {
            view,
            values: vec![SqlValue::Null; view.columns.len()],
        }
    }

    /// Sets column `name`.
    ///
    /// # Panics
    ///
    /// When the view has no column `name`, or the value's type is not the column's: a programming error
    /// that must not shift other columns silently.
    pub fn set(&mut self, name: &str, value: impl Into<Cell>) -> &mut Row {
        let Some(i) = self.view.columns.iter().position(|c| c.name == name) else {
            panic!("{} has no column `{name}`", self.view.name);
        };
        let value = value.into().0;
        let ok = matches!(
            (&value, self.view.columns[i].ty),
            (SqlValue::Null, _)
                | (SqlValue::Text(_), ColType::Text)
                | (SqlValue::Integer(_), ColType::Integer)
        );
        assert!(ok, "{}.{name}: wrong value type {value:?}", self.view.name);
        self.values[i] = value;
        self
    }

    /// The values in column order.
    pub fn finish(self) -> Vec<SqlValue> {
        self.values
    }
}

/// A column value.
pub(crate) struct Cell(pub SqlValue);

impl From<String> for Cell {
    fn from(s: String) -> Cell {
        Cell(SqlValue::Text(s))
    }
}

impl From<&str> for Cell {
    fn from(s: &str) -> Cell {
        Cell(SqlValue::Text(s.to_owned()))
    }
}

impl From<&String> for Cell {
    fn from(s: &String) -> Cell {
        Cell(SqlValue::Text(s.clone()))
    }
}

impl From<i64> for Cell {
    fn from(v: i64) -> Cell {
        Cell(SqlValue::Integer(v))
    }
}

impl From<u32> for Cell {
    fn from(v: u32) -> Cell {
        Cell(SqlValue::Integer(i64::from(v)))
    }
}

impl From<u16> for Cell {
    fn from(v: u16) -> Cell {
        Cell(SqlValue::Integer(i64::from(v)))
    }
}

impl From<u8> for Cell {
    fn from(v: u8) -> Cell {
        Cell(SqlValue::Integer(i64::from(v)))
    }
}

impl From<usize> for Cell {
    fn from(v: usize) -> Cell {
        Cell(SqlValue::Integer(i64::try_from(v).unwrap_or(i64::MAX)))
    }
}

impl From<bool> for Cell {
    fn from(v: bool) -> Cell {
        Cell(SqlValue::Integer(i64::from(v)))
    }
}

impl<T: Into<Cell>> From<Option<T>> for Cell {
    fn from(v: Option<T>) -> Cell {
        v.map_or(Cell(SqlValue::Null), Into::into)
    }
}

/// `None` for an empty string.
pub(crate) fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// A list as a JSON array.
pub(crate) fn json_array<S: Serialize>(items: &[S]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_owned())
}

/// The lowercase serialised name of a schema enum (`schematic`, `not_fitted`), or its JSON text for a value
/// that does not serialise to a string.
pub(crate) fn enum_text<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// The JSON of a multilingual value.
pub(crate) fn ml_json(ml: &MlText) -> String {
    serde_json::to_string(ml).unwrap_or_default()
}

/// A cross-section as (value, unit): `("0.75", "mm²")`, `("14", "AWG")`, `("250", "kcmil")`.
pub(crate) fn csa_parts(c: &Csa) -> Option<(String, &'static str)> {
    match c {
        Csa::Mm2(d) => Some((d.to_string(), "mm\u{00B2}")),
        Csa::Awg(n) => Some((n.to_string(), "AWG")),
        Csa::Kcmil(d) => Some((d.to_string(), "kcmil")),
        Csa::Unknown(_) => None,
    }
}

/// The colour when every code is an IEC 60757 code.
pub(crate) fn colour_iec(c: &ColourSpec) -> Option<String> {
    if c.codes().iter().all(|c| matches!(c, ColourCode::Iec(_))) {
        Some(ew_derive::colour_raw(c))
    } else {
        None
    }
}
