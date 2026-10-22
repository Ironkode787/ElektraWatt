// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sandbox every consumer reads the projection through (ADR-0017 decision 4, doc 17 §6.2).
//!
//! - **Connection.** A separate connection per consumer, opened `SQLITE_OPEN_READONLY` on a read-only
//!   deserialised copy of the projection image (`sqlite3_deserialize` with `SQLITE_DESERIALIZE_READONLY`),
//!   with `SQLITE_DBCONFIG_DEFENSIVE`, `trusted_schema = OFF`, triggers off, `query_only = 1` and extension
//!   loading off (the bundled build never enables it on a connection).
//! - **Statement gate.** Exactly one statement per call, starting with `SELECT`, `WITH` or `VALUES`, for
//!   which `sqlite3_stmt_readonly()` is true. This refuses `VACUUM INTO` and `WITH … INSERT` before they run.
//! - **Authorizer.** It allows `SELECT`, `RECURSIVE`, the allowlisted functions, and `READ` of a `t_*` column
//!   only when the innermost responsible view is its own `api_v1_*` view; reads of the table-valued JSON
//!   functions are allowed. It denies everything else: `ATTACH`, `DETACH`, every `PRAGMA`, DDL, DML,
//!   transactions and savepoints, `ANALYZE`, `REINDEX`, direct reads of `t_*` and of the schema table.
//! - **Limits** (`SQLITE_LIMIT_*`, [`LIMITS`]), a **deadline** through the progress handler, and **result
//!   caps** in rows and bytes per [`Profile`].

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

use crate::error::SqlError;
use crate::functions::{self, CURRENT_FUNCTIONS, DATE_FUNCTIONS};
use crate::result::{ResultSet, Value};

/// The limits and the deadline of one kind of consumer (doc 17 §6.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Profile {
    /// The consumer, for messages (`ew sql`, `report`).
    pub name: &'static str,
    /// The deadline of one statement, from the start of its first step to its last row.
    pub deadline: Duration,
    /// The most rows a result may have.
    pub max_rows: usize,
    /// The most bytes a result may hold (text and blob lengths, 8 per number).
    pub max_bytes: usize,
}

const MIB: usize = 1024 * 1024;

impl Profile {
    /// `ew sql`: 60 s (`--timeout`), 1,000,000 rows, 256 MiB.
    pub const EW_SQL: Profile = Profile {
        name: "ew sql",
        deadline: Duration::from_secs(60),
        max_rows: 1_000_000,
        max_bytes: 256 * MIB,
    };
    /// A user report block: 2 s, 100,000 rows (doc 09 §3.5); the byte cap of `ew sql`.
    pub const REPORT: Profile = Profile {
        name: "report",
        deadline: Duration::from_secs(2),
        max_rows: 100_000,
        max_bytes: 256 * MIB,
    };
    /// An SQL rule: 10 s; the row and byte caps of `ew sql`.
    pub const RULE: Profile = Profile {
        name: "rule",
        deadline: Duration::from_secs(10),
        max_rows: 1_000_000,
        max_bytes: 256 * MIB,
    };
    /// The MCP `sql_query` tool: 5 s, 1,000 rows, 1 MiB.
    pub const MCP: Profile = Profile {
        name: "mcp",
        deadline: Duration::from_secs(5),
        max_rows: 1_000,
        max_bytes: MIB,
    };

    /// The longest deadline a profile may have: one day.
    pub const MAX_DEADLINE: Duration = Duration::from_secs(24 * 60 * 60);

    /// The same profile with another deadline (`ew sql --timeout`), at most [`Profile::MAX_DEADLINE`].
    #[must_use]
    pub fn with_deadline(mut self, deadline: Duration) -> Profile {
        self.deadline = deadline.min(Profile::MAX_DEADLINE);
        self
    }

    /// The instant the deadline of a statement started at `start` passes; a deadline beyond
    /// [`Profile::MAX_DEADLINE`] counts as that maximum (a hand-built profile cannot overflow the clock).
    fn deadline_from(&self, start: Instant) -> Instant {
        let d = self.deadline.min(Profile::MAX_DEADLINE);
        start.checked_add(d).unwrap_or(start)
    }
}

/// The `SQLITE_LIMIT_*` values of every consumer connection (doc 17 §6.2). `VDBE_OP` is a prepare-time
/// program-size cap whose enforcement by current SQLite is (verify); no property rests on it, the deadline
/// bounds the runtime.
pub const LIMITS: [(Limit, i32); 12] = [
    (Limit::SQLITE_LIMIT_LENGTH, 16 * 1024 * 1024),
    (Limit::SQLITE_LIMIT_SQL_LENGTH, 256 * 1024),
    (Limit::SQLITE_LIMIT_COLUMN, 256),
    (Limit::SQLITE_LIMIT_EXPR_DEPTH, 200),
    (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 50),
    (Limit::SQLITE_LIMIT_VDBE_OP, 250_000),
    (Limit::SQLITE_LIMIT_FUNCTION_ARG, 32),
    (Limit::SQLITE_LIMIT_ATTACHED, 0),
    (Limit::SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 1_000),
    (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 256),
    (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 0),
    (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
];

/// The VM steps between two calls of the progress handler that checks the deadline.
const PROGRESS_STEPS: i32 = 10_000;

/// The functions a statement may call: SQLite's core scalar, aggregate, window, math and JSON functions, the
/// overridden date functions and `ew_ml`. `random`, `randomblob`, `load_extension`, `sqlite_*` diagnostics,
/// `changes`-style connection state and everything not listed are denied at prepare time.
const FUNCTIONS: &[&str] = &[
    // Core scalar functions.
    "abs",
    "char",
    "coalesce",
    "concat",
    "concat_ws",
    "format",
    "glob",
    "hex",
    "if",
    "ifnull",
    "iif",
    "instr",
    "length",
    "like",
    "likelihood",
    "likely",
    "lower",
    "ltrim",
    "max",
    "min",
    "nullif",
    "octet_length",
    "printf",
    "quote",
    "replace",
    "round",
    "rtrim",
    "sign",
    "soundex",
    "substr",
    "substring",
    "trim",
    "typeof",
    "unhex",
    "unicode",
    "unlikely",
    "upper",
    "zeroblob",
    // Aggregates.
    "avg",
    "count",
    "group_concat",
    "string_agg",
    "sum",
    "total",
    // Window functions.
    "row_number",
    "rank",
    "dense_rank",
    "percent_rank",
    "cume_dist",
    "ntile",
    "lag",
    "lead",
    "first_value",
    "last_value",
    "nth_value",
    // Math functions.
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atan2",
    "atanh",
    "ceil",
    "ceiling",
    "cos",
    "cosh",
    "degrees",
    "exp",
    "floor",
    "ln",
    "log",
    "log10",
    "log2",
    "mod",
    "pi",
    "pow",
    "power",
    "radians",
    "sin",
    "sinh",
    "sqrt",
    "tan",
    "tanh",
    "trunc",
    // JSON functions (the table-valued `json_each` and `json_tree` are reads, see the authorizer).
    "json",
    "jsonb",
    "json_array",
    "jsonb_array",
    "json_array_length",
    "json_error_position",
    "json_extract",
    "jsonb_extract",
    "json_insert",
    "jsonb_insert",
    "json_object",
    "jsonb_object",
    "json_patch",
    "jsonb_patch",
    "json_pretty",
    "json_remove",
    "jsonb_remove",
    "json_replace",
    "jsonb_replace",
    "json_set",
    "jsonb_set",
    "json_type",
    "json_valid",
    "json_quote",
    "json_group_array",
    "jsonb_group_array",
    "json_group_object",
    "jsonb_group_object",
    "json_each",
    "json_tree",
    // Host functions.
    "ew_ml",
];

/// The table-valued functions whose rows a statement may read.
const TABLE_FUNCTIONS: [&str; 2] = ["json_each", "json_tree"];

/// Whether a name read without a schema can still be a schema object or an eponymous virtual table rather
/// than the work table of a common table expression: SQLite reports a column-less read (`count(*)`) of a
/// real table without its schema too.
fn schema_object(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("sqlite_")
        || lower.starts_with("pragma_")
        || lower == "dbstat"
        || lower.starts_with("t_")
}

fn function_allowed(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    FUNCTIONS.contains(&lower.as_str())
        || DATE_FUNCTIONS.iter().any(|(f, _)| *f == lower)
        || CURRENT_FUNCTIONS.contains(&lower.as_str())
}

/// What the authorizer remembers while one statement is prepared.
#[derive(Debug, Default)]
struct AuthState {
    /// The `api_v1_*` views of the image (read once, before the authorizer is installed).
    views: BTreeSet<String>,
    /// The internal tables this statement reads through a view.
    via_views: BTreeSet<String>,
    /// The first action denied.
    denied: Option<String>,
}

impl AuthState {
    /// The decision for one action.
    ///
    /// `READ` rules (doc 17 §6.2): a column of an `api_v1_*` view is readable; a column of a `t_*` table only
    /// when the innermost responsible view is that table's own `api_v1_*` view of the image. SQLite flattens a view into
    /// the outer query, and a query that reads no column of it (`count(*)`) then reads the table itself
    /// with an empty column name from top-level SQL: that is allowed only for a table the same statement
    /// already reads through a view.
    ///
    /// A `t_x` table is readable only through its own view `api_v1_x`. The rule guards the contract, not
    /// secrecy: every column of `t_x` is also a column of `api_v1_x`. A common table expression named after
    /// a view is reported as that view by SQLite's authorizer, so `WITH api_v1_x AS (SELECT … FROM t_x)` can
    /// read the one table behind the view it shadows; it sees the view's columns and `rowid` (the insertion
    /// order, the view's `seq` where it has one), never another table.
    fn decide(&mut self, ctx: &AuthContext<'_>) -> Result<(), String> {
        match ctx.action {
            AuthAction::Select | AuthAction::Recursive => Ok(()),
            AuthAction::Read {
                table_name,
                column_name,
            } => {
                if table_name.starts_with("t_") {
                    // Only the view of the same name reads a table: `t_wire` through `api_v1_wire`.
                    let through_view = ctx.accessor.is_some_and(|v| {
                        self.views.contains(v) && v.strip_prefix("api_v1_") == table_name.strip_prefix("t_")
                    });
                    if through_view {
                        self.via_views.insert(table_name.to_owned());
                        Ok(())
                    } else if column_name.is_empty() && self.via_views.contains(table_name) {
                        Ok(())
                    } else {
                        Err(format!(
                            "read of the internal table {table_name}; query the api_v1_* views"
                        ))
                    }
                } else if self.views.contains(table_name)
                    || TABLE_FUNCTIONS.contains(&table_name)
                    || (ctx.database_name.is_none() && !schema_object(table_name))
                {
                    // A view, a JSON table-valued function, or the work table of a recursive common table
                    // expression (it belongs to no schema).
                    Ok(())
                } else {
                    Err(format!("read of {table_name}"))
                }
            }
            AuthAction::Function { function_name } => {
                if function_allowed(function_name) {
                    Ok(())
                } else {
                    Err(format!("function {function_name}"))
                }
            }
            AuthAction::Attach { .. } => Err("ATTACH".to_owned()),
            AuthAction::Detach { .. } => Err("DETACH".to_owned()),
            AuthAction::Pragma { pragma_name, .. } => Err(format!("PRAGMA {pragma_name}")),
            AuthAction::Transaction { .. } => Err("transaction".to_owned()),
            AuthAction::Savepoint { .. } => Err("SAVEPOINT".to_owned()),
            AuthAction::Analyze { .. } => Err("ANALYZE".to_owned()),
            AuthAction::Reindex { .. } => Err("REINDEX".to_owned()),
            AuthAction::Insert { table_name } => Err(format!("INSERT into {table_name}")),
            AuthAction::Update { table_name, .. } => Err(format!("UPDATE of {table_name}")),
            AuthAction::Delete { table_name } => Err(format!("DELETE from {table_name}")),
            _ => Err("DDL or another statement that is not a query".to_owned()),
        }
    }
}

/// A sandboxed, read-only connection to one projection image.
pub struct Reader {
    conn: Connection,
    profile: Profile,
    /// The deadline of the running statement; `None` between statements.
    deadline: Arc<Mutex<Option<Instant>>>,
    /// The authorizer's per-statement state.
    auth: Arc<Mutex<AuthState>>,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

fn sql_err(e: &rusqlite::Error) -> SqlError {
    SqlError::Sql(e.to_string())
}

impl Reader {
    /// Opens a sandboxed connection on a read-only copy of `image` (an SQLite database image).
    // The deadline of a query is a resource limit on the monotonic clock: a query that finishes inside it returns
    // the same rows on every run, one that does not is refused with a timeout; no timestamp is in a result.
    #[allow(clippy::disallowed_methods)]
    pub(crate) fn open(image: &[u8], profile: Profile) -> Result<Reader, SqlError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut conn = Connection::open_in_memory_with_flags(flags).map_err(|e| sql_err(&e))?;
        conn.deserialize_read_exact("main", image, image.len(), true)
            .map_err(|e| sql_err(&e))?;
        configure(&conn).map_err(|e| sql_err(&e))?;
        let deadline: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let auth = Arc::new(Mutex::new(AuthState {
            views: view_names(&conn).map_err(|e| sql_err(&e))?,
            ..AuthState::default()
        }));
        {
            let deadline = Arc::clone(&deadline);
            conn.progress_handler(
                PROGRESS_STEPS,
                Some(move || {
                    deadline
                        .lock()
                        .map(|d| d.is_some_and(|d| Instant::now() >= d))
                        .unwrap_or(true)
                }),
            )
            .map_err(|e| sql_err(&e))?;
        }
        {
            let auth = Arc::clone(&auth);
            conn.authorizer(Some(move |ctx: AuthContext<'_>| {
                let Ok(mut state) = auth.lock() else {
                    return Authorization::Deny;
                };
                match state.decide(&ctx) {
                    Ok(()) => Authorization::Allow,
                    Err(what) => {
                        state.denied.get_or_insert(what);
                        Authorization::Deny
                    }
                }
            }))
            .map_err(|e| sql_err(&e))?;
        }
        Ok(Reader {
            conn,
            profile,
            deadline,
            auth,
        })
    }

    /// The profile of this reader.
    pub fn profile(&self) -> Profile {
        self.profile
    }

    /// Runs one query under the sandbox and returns all its rows.
    ///
    /// # Errors
    ///
    /// [`SqlError`]: the statement gate, the authorizer, the deadline, the result caps or SQLite refused it.
    // The start of the query's deadline (see `open`): a resource limit, not a value of the result.
    #[allow(clippy::disallowed_methods)]
    pub fn query(&self, sql: &str) -> Result<ResultSet, SqlError> {
        let word = first_word(sql).ok_or(SqlError::Empty)?;
        if !matches!(word.to_ascii_uppercase().as_str(), "SELECT" | "WITH" | "VALUES") {
            return Err(SqlError::NotAQuery(word));
        }
        self.reset_denied();
        let mut stmt = match self.conn.prepare(sql) {
            Ok(s) => s,
            Err(e) => return Err(self.prepare_error(&e)),
        };
        if !stmt.readonly() {
            return Err(SqlError::NotReadOnly);
        }
        let columns: Vec<String> = stmt.column_names().iter().map(|c| (*c).to_owned()).collect();
        let n = columns.len();
        self.set_deadline(Some(self.profile.deadline_from(Instant::now())));
        let result = self.collect(&mut stmt, n);
        self.set_deadline(None);
        result.map(|rows| ResultSet { columns, rows })
    }

    fn collect(&self, stmt: &mut rusqlite::Statement<'_>, n: usize) -> Result<Vec<Vec<Value>>, SqlError> {
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        let mut cursor = stmt.query([]).map_err(|e| self.step_error(&e))?;
        loop {
            let row = match cursor.next() {
                Ok(Some(r)) => r,
                Ok(None) => break,
                Err(e) => return Err(self.step_error(&e)),
            };
            if rows.len() == self.profile.max_rows {
                return Err(SqlError::RowLimit {
                    rows: self.profile.max_rows,
                });
            }
            let mut values = Vec::with_capacity(n);
            for i in 0..n {
                let v = match row.get_ref(i).map_err(|e| self.step_error(&e))? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(i) => Value::Integer(i),
                    ValueRef::Real(r) => Value::Real(r),
                    ValueRef::Text(t) => Value::Text(String::from_utf8_lossy(t).into_owned()),
                    ValueRef::Blob(b) => Value::Blob(b.to_vec()),
                };
                bytes = bytes.saturating_add(v.weight());
                values.push(v);
            }
            if bytes > self.profile.max_bytes {
                return Err(SqlError::ByteLimit {
                    bytes: self.profile.max_bytes,
                });
            }
            rows.push(values);
        }
        Ok(rows)
    }

    fn reset_denied(&self) {
        if let Ok(mut a) = self.auth.lock() {
            a.denied = None;
            a.via_views.clear();
        }
    }

    fn take_denied(&self) -> Option<String> {
        self.auth.lock().ok().and_then(|mut a| a.denied.take())
    }

    fn set_deadline(&self, d: Option<Instant>) {
        if let Ok(mut slot) = self.deadline.lock() {
            *slot = d;
        }
    }

    fn prepare_error(&self, e: &rusqlite::Error) -> SqlError {
        if let Some(what) = self.take_denied() {
            return SqlError::Denied(what);
        }
        match e {
            rusqlite::Error::MultipleStatement => SqlError::MultipleStatements,
            other => sql_err(other),
        }
    }

    fn step_error(&self, e: &rusqlite::Error) -> SqlError {
        if let rusqlite::Error::SqliteFailure(f, _) = e {
            if f.code == rusqlite::ErrorCode::OperationInterrupted {
                return SqlError::Deadline {
                    ms: self.profile.deadline.as_millis(),
                };
            }
        }
        if let Some(what) = self.take_denied() {
            return SqlError::Denied(what);
        }
        sql_err(e)
    }
}

/// The connection settings that hold before the authorizer is installed (the authorizer denies `PRAGMA`).
fn configure(conn: &Connection) -> rusqlite::Result<()> {
    use rusqlite::config::DbConfig;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FTS3_TOKENIZER, false)?;
    conn.pragma_update(None, "query_only", 1)?;
    for (limit, value) in LIMITS {
        conn.set_limit(limit, value)?;
    }
    functions::register(conn)
}

/// The names of the `api_v1_*` views of the image.
fn view_names(conn: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'view' AND substr(name, 1, 7) = 'api_v1_'")?;
    let names = stmt.query_map([], |r| r.get::<_, String>(0))?;
    names.collect()
}

/// The first word of `sql` after white space and comments, `None` when there is none.
fn first_word(sql: &str) -> Option<String> {
    let mut rest = sql;
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("--") {
            rest = r.find('\n').map_or("", |i| &r[i + 1..]);
        } else if let Some(r) = rest.strip_prefix("/*") {
            rest = r.find("*/").map_or("", |i| &r[i + 2..]);
        } else {
            break;
        }
    }
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if word.is_empty() {
        rest.chars().next().map(|c| c.to_string())
    } else {
        Some(word)
    }
}

#[cfg(test)]
mod tests {
    use super::first_word;

    #[test]
    fn first_word_skips_comments() {
        assert_eq!(first_word("  select 1").as_deref(), Some("select"));
        assert_eq!(
            first_word("-- x\n/* y */ WITH a AS (SELECT 1) SELECT * FROM a").as_deref(),
            Some("WITH")
        );
        assert_eq!(first_word("-- only a comment"), None);
        assert_eq!(first_word("   "), None);
        assert_eq!(first_word("(SELECT 1)").as_deref(), Some("("));
    }
}
