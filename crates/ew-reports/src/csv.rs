// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CSV text of a report block's rows (doc 09 §1.3; `ew export cables|bom`, the golden `expected/csv/`).
//!
//! One writer for every consumer: the CLI exports, the acceptance runner and the golden files are all this
//! function over [`RowSet`]. The dialect is the one of the golden README ("CSV dialect"): UTF-8, LF line ends,
//! a header line, a field quoted exactly when it contains `,`, `"`, LF or CR or begins or ends with a space,
//! quotes doubled. The first column is `row`: `data` for a data row and `heading` for every other kind; the
//! other columns are the block's columns, headed by their neutral labels and holding the printed cells
//! ([`render`](crate::render::render) in neutral language).

use ew_derive::Q;

use crate::render::{RenderOptions, render};
use crate::rowset::{RowKind, RowSet};

/// One field of the golden CSV dialect.
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// The printed cells of row `i` of `set` (neutral language, no page).
pub fn row_texts(q: &Q, set: &RowSet, i: usize) -> Vec<String> {
    set.rows[i]
        .cells
        .iter()
        .map(|c| render(q, c, &RenderOptions::default()))
        .collect()
}

/// The printed rows of a block as CSV: the header (`row` and the column labels), then `data` or `heading` and
/// the printed cells of every row.
pub fn rows_csv(q: &Q, set: &RowSet) -> String {
    let mut out = String::new();
    let header: Vec<String> = std::iter::once("row".to_owned())
        .chain(
            set.columns
                .iter()
                .map(|c| csv_field(c.label.as_neutral().unwrap_or(&c.key))),
        )
        .collect();
    out.push_str(&header.join(","));
    out.push('\n');
    for i in 0..set.rows.len() {
        let kind = if set.rows[i].kind == RowKind::Data {
            "data"
        } else {
            "heading"
        };
        let cells: Vec<String> = std::iter::once(kind.to_owned())
            .chain(row_texts(q, set, i).iter().map(|t| csv_field(t)))
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_quoted_exactly_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field(" lead"), "\" lead\"");
        assert_eq!(csv_field("trail "), "\"trail \"");
        assert_eq!(csv_field("two\nlines"), "\"two\nlines\"");
        assert_eq!(csv_field(""), "");
    }
}
