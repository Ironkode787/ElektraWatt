// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rows of the internal tables, filled by column name, and the value conventions of doc 17 §6.1.

use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{ColourCode, ColourSpec, Csa};
use ew_derive::ew_units::Dec3;
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

/// The neutral text of a multilingual value, else its first language's text.
pub(crate) fn ml_text(ml: &MlText) -> String {
    ml.as_neutral()
        .or_else(|| ml.languages().next().and_then(|l| ml.get(l)))
        .unwrap_or_default()
        .to_owned()
}

/// The JSON of a multilingual value.
pub(crate) fn ml_json(ml: &MlText) -> String {
    serde_json::to_string(ml).unwrap_or_default()
}

/// A decimal as exact text: `0.75`, `24`, `-1.5`.
pub(crate) fn decimal(v: Dec3) -> String {
    let abs = v.0.unsigned_abs();
    let (int, frac) = (abs / 1000, abs % 1000);
    let mut s = String::new();
    if v.0 < 0 {
        s.push('-');
    }
    s.push_str(&int.to_string());
    if frac != 0 {
        s.push('.');
        s.push_str(format!("{frac:03}").trim_end_matches('0'));
    }
    s
}

/// A cross-section as (value, unit): `("0.75", "mm²")`, `("14", "AWG")`, `("250", "kcmil")`.
pub(crate) fn csa_parts(c: &Csa) -> Option<(String, &'static str)> {
    match c {
        Csa::Mm2(d) => Some((decimal(*d), "mm\u{00B2}")),
        Csa::Awg(n) => Some((n.to_string(), "AWG")),
        Csa::Kcmil(d) => Some((decimal(*d), "kcmil")),
        Csa::Unknown(_) => None,
    }
}

/// A colour as printed: the codes concatenated (`GNYE`).
pub(crate) fn colour_text(c: &ColourSpec) -> String {
    c.codes()
        .iter()
        .map(|c| match c {
            ColourCode::Iec(i) => i.as_str().to_owned(),
            ColourCode::Company(i) | ColourCode::Foreign(i) => i.as_str().to_owned(),
            ColourCode::Unknown(v) => v.to_string(),
        })
        .collect()
}

/// The colour when every code is an IEC 60757 code.
pub(crate) fn colour_iec(c: &ColourSpec) -> Option<String> {
    if c.codes().iter().all(|c| matches!(c, ColourCode::Iec(_))) {
        Some(colour_text(c))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_are_exact_text() {
        assert_eq!(decimal(Dec3(750)), "0.75");
        assert_eq!(decimal(Dec3(24_000)), "24");
        assert_eq!(decimal(Dec3(-1_500)), "-1.5");
        assert_eq!(decimal(Dec3(1)), "0.001");
    }
}
