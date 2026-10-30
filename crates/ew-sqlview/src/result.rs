// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The result of a query: column names and rows of SQLite values, written as CSV (the golden CSV dialect) or
//! JSON.

use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One SQLite value.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    /// `NULL`.
    Null,
    /// An integer.
    Integer(i64),
    /// A floating-point number (a computed value such as `CAST(voltage AS REAL)`; the projection itself
    /// stores none, quantities are exact decimal text).
    Real(f64),
    /// Text.
    Text(String),
    /// A blob.
    Blob(Vec<u8>),
}

impl Value {
    /// The value as a CSV cell or plain text: `NULL` is empty, a real prints in Rust's shortest round-trip
    /// form, a blob as `x'…'` hex.
    pub fn text(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Integer(i) => i.to_string(),
            Value::Real(r) => real_text(*r),
            Value::Text(s) => s.clone(),
            Value::Blob(b) => {
                let mut s = String::with_capacity(3 + 2 * b.len());
                s.push_str("x'");
                for byte in b {
                    let _ = write!(s, "{byte:02x}");
                }
                s.push('\'');
                s
            }
        }
    }

    /// The bytes the value counts towards a byte cap: the text or blob length, 8 for a number, 0 for `NULL`.
    pub(crate) fn weight(&self) -> usize {
        match self {
            Value::Null => 0,
            Value::Integer(_) | Value::Real(_) => 8,
            Value::Text(s) => s.len(),
            Value::Blob(b) => b.len(),
        }
    }
}

fn real_text(r: f64) -> String {
    if r.is_finite() && r == r.trunc() && r.abs() < 1e15 {
        // `24.0`, as SQLite prints a whole real.
        format!("{r:.1}")
    } else {
        format!("{r}")
    }
}

/// The rows a query returned.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ResultSet {
    /// The column names, as the statement names them (duplicates allowed: a list has two `CP` columns).
    pub columns: Vec<String>,
    /// The rows, each with one value per column.
    pub rows: Vec<Vec<Value>>,
}

/// A field of the golden CSV dialect (`golden/J-E-2416/README.md`, "CSV dialect"): quoted when it holds a
/// comma, a quote or a line break, or starts or ends with a space; quotes doubled.
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

impl ResultSet {
    /// The result as CSV in the golden dialect: a header line with the column names, then one line per row,
    /// every line ending in `\n`. `NULL` is an empty field.
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        let line = |cells: Vec<String>| cells.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(",");
        out.push_str(&line(self.columns.clone()));
        out.push('\n');
        for row in &self.rows {
            out.push_str(&line(row.iter().map(Value::text).collect()));
            out.push('\n');
        }
        out
    }

    /// The result as JSON (`ew.sql-result/1`, doc 17 §3.1): `{"$schema", "columns", "rows"}` with the rows as
    /// arrays of typed values (numbers stay numbers, `NULL` is `null`, a blob is its `x'…'` text).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self.to_result()).unwrap_or(serde_json::Value::Null)
    }

    /// The result as its typed JSON result ([`SqlResult`]).
    pub fn to_result(&self) -> SqlResult {
        SqlResult {
            schema: SQL_RESULT_SCHEMA.to_owned(),
            columns: self.columns.clone(),
            rows: self
                .rows
                .iter()
                .map(|r| r.iter().map(SqlCell::of).collect())
                .collect(),
        }
    }
}

/// The schema name of [`SqlResult`].
pub const SQL_RESULT_SCHEMA: &str = "ew.sql-result/1";

/// The JSON result of `ew sql` (`ew.sql-result/1`, doc 17 §3.1, WP-S1.22 D5): the type is the published schema
/// (`bin/ew/schemas/ew.sql-result-1.schema.json`). Members print in byte-wise key order (`$schema` first).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SqlResult {
    /// [`SQL_RESULT_SCHEMA`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The column names, as the statement names them (duplicates allowed).
    pub columns: Vec<String>,
    /// The rows, each with one value per column.
    pub rows: Vec<Vec<SqlCell>>,
}

/// One value of a [`SqlResult`] row: numbers stay numbers, `NULL` is `null`, a blob is its `x'…'` text, a real
/// that is not finite is `null`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SqlCell {
    /// `NULL`.
    Null,
    /// An integer.
    Integer(i64),
    /// A floating-point number.
    Real(f64),
    /// Text (or a blob as `x'…'`).
    Text(String),
}

impl SqlCell {
    fn of(v: &Value) -> SqlCell {
        match v {
            Value::Null => SqlCell::Null,
            Value::Integer(i) => SqlCell::Integer(*i),
            Value::Real(r) if r.is_finite() => SqlCell::Real(*r),
            Value::Real(_) => SqlCell::Null,
            Value::Text(_) | Value::Blob(_) => SqlCell::Text(v.text()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_like_the_golden_dialect() {
        let set = ResultSet {
            columns: vec!["row".into(), "CP".into(), "CP".into()],
            rows: vec![
                vec![
                    Value::Text("data".into()),
                    Value::Text("101, 102".into()),
                    Value::Null,
                ],
                vec![
                    Value::Integer(3),
                    Value::Text("say \"hi\"".into()),
                    Value::Real(24.0),
                ],
            ],
        };
        assert_eq!(
            set.to_csv(),
            "row,CP,CP\ndata,\"101, 102\",\n3,\"say \"\"hi\"\"\",24.0\n"
        );
    }

    #[test]
    fn json_keeps_types() {
        let set = ResultSet {
            columns: vec!["a".into(), "b".into()],
            rows: vec![vec![Value::Integer(1), Value::Null]],
        };
        assert_eq!(
            set.to_json().to_string(),
            r#"{"$schema":"ew.sql-result/1","columns":["a","b"],"rows":[[1,null]]}"#
        );
    }

    #[test]
    fn blob_and_real_text() {
        assert_eq!(Value::Blob(vec![0, 255]).text(), "x'00ff'");
        assert_eq!(Value::Real(0.75).text(), "0.75");
        assert_eq!(Value::Real(-2.0).text(), "-2.0");
    }
}
