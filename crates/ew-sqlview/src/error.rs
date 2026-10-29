// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The errors of the projection: building it, and running SQL under the sandbox.

use std::fmt;

/// Why a statement did not run or did not complete under the sandbox (doc 17 §6.2).
///
/// Every variant is a "could not complete" outcome for `ew sql` (exit code 2, doc 17 §3.2): content refused,
/// a limit or deadline hit, or an SQL error.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SqlError {
    /// The text is empty or holds only comments.
    Empty,
    /// The statement does not start with `SELECT`, `WITH` or `VALUES`; the first word is given.
    NotAQuery(String),
    /// More than one statement (a non-empty tail after the first).
    MultipleStatements,
    /// `sqlite3_stmt_readonly()` is false for the statement (`WITH … INSERT`, `VACUUM INTO`).
    NotReadOnly,
    /// The authorizer refused an action; the text names it (`ATTACH`, `PRAGMA table_info`, `read of t_device`,
    /// `function random`).
    Denied(String),
    /// The deadline of the consumer's profile passed (doc 17 §6.2: report 2 s, rule 10 s, MCP 5 s, `ew sql`
    /// 60 s).
    Deadline {
        /// The deadline in milliseconds.
        ms: u128,
    },
    /// The result has more rows than the profile allows.
    RowLimit {
        /// The cap.
        rows: usize,
    },
    /// The result is larger than the profile allows.
    ByteLimit {
        /// The cap in bytes.
        bytes: usize,
    },
    /// Any other error SQLite reports (syntax, unknown column, a function that rejected its arguments such
    /// as `date('now')`, an `SQLITE_LIMIT_*` limit).
    Sql(String),
}

impl fmt::Display for SqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlError::Empty => write!(f, "no SQL statement given"),
            SqlError::NotAQuery(word) => write!(
                f,
                "statement denied by the projection sandbox ({}): only SELECT, WITH or VALUES may run",
                word.to_uppercase()
            ),
            SqlError::MultipleStatements => write!(
                f,
                "statement denied by the projection sandbox: exactly one statement per call"
            ),
            SqlError::NotReadOnly => write!(
                f,
                "statement denied by the projection sandbox: the statement is not read-only"
            ),
            SqlError::Denied(what) => write!(f, "statement denied by the projection sandbox ({what})"),
            SqlError::Deadline { ms } => write!(f, "the statement passed its deadline of {ms} ms"),
            SqlError::RowLimit { rows } => write!(f, "the result exceeds the limit of {rows} rows"),
            SqlError::ByteLimit { bytes } => write!(f, "the result exceeds the limit of {bytes} bytes"),
            SqlError::Sql(msg) => write!(f, "SQL error: {msg}"),
        }
    }
}

impl std::error::Error for SqlError {}

/// Why the projection could not be built: SQLite failed while the tables were written (out of memory, a
/// limit). The model itself never makes a build fail.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BuildError(pub String);

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the SQL projection could not be built: {}", self.0)
    }
}

impl std::error::Error for BuildError {}

impl From<rusqlite::Error> for BuildError {
    fn from(e: rusqlite::Error) -> BuildError {
        BuildError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::SqlError;

    /// The refusal texts users and scripts see on stderr (`ew sql` exits 2 with them) name the denied statement in
    /// parentheses. The CLI test of `ATTACH` was folded away (test audit g6), so the text is pinned here, next to
    /// its source.
    #[test]
    fn refusals_name_the_denied_statement() {
        assert_eq!(
            SqlError::NotAQuery("attach".into()).to_string(),
            "statement denied by the projection sandbox (ATTACH): only SELECT, WITH or VALUES may run"
        );
        assert_eq!(
            SqlError::Denied("ATTACH".into()).to_string(),
            "statement denied by the projection sandbox (ATTACH)"
        );
    }
}
