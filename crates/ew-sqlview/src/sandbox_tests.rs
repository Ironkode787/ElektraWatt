// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The security tests of the sandbox (doc 18 §11.5 "SQL sandbox", "SQL statement gate"; NFR-SEC-06,
//! TM-05): hostile SQL is refused by the gate or the authorizer, or stopped by the limits, the deadline and
//! the result caps. They run on a small hand-built image with one internal table behind one view.

use std::time::Duration;

use crate::error::SqlError;
use crate::result::Value;
use crate::sandbox::{Profile, Reader};

/// An image with `t_item(name, n)` behind `api_v1_item` and five rows.
fn image() -> Vec<u8> {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t_item (name TEXT, n INTEGER);
         CREATE VIEW api_v1_item AS SELECT name, n FROM t_item;
         INSERT INTO t_item VALUES ('X10', 10), ('X2', 2), ('x1', 1), ('X1', 1), ('-X1031:2', 2);
         CREATE TABLE t_other (code TEXT, hidden TEXT);
         CREATE VIEW api_v1_other AS SELECT code FROM t_other;
         INSERT INTO t_other VALUES ('a', 'not in the view');",
    )
    .unwrap();
    conn.serialize("main").unwrap().to_vec()
}

fn reader(profile: Profile) -> Reader {
    Reader::open(&image(), profile).unwrap()
}

fn ew_sql() -> Reader {
    reader(Profile::EW_SQL)
}

fn texts(r: &Reader, sql: &str) -> Vec<String> {
    r.query(sql)
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
        .rows
        .iter()
        .map(|row| row.iter().map(Value::text).collect::<Vec<_>>().join("|"))
        .collect()
}

fn denied(r: &Reader, sql: &str) -> String {
    match r.query(sql) {
        Err(SqlError::Denied(what)) => what,
        other => panic!("{sql}: expected a denial, got {other:?}"),
    }
}

#[test]
fn views_are_readable_and_internal_tables_are_not() {
    let r = ew_sql();
    assert_eq!(texts(&r, "SELECT count(*) FROM api_v1_item"), ["5"]);
    assert_eq!(
        texts(
            &r,
            "WITH a AS (SELECT name FROM api_v1_item WHERE n = 1) SELECT count(*) FROM a"
        ),
        ["2"]
    );
    assert!(denied(&r, "SELECT * FROM t_item").contains("t_item"));
    assert!(denied(&r, "SELECT name FROM api_v1_item UNION SELECT name FROM t_item").contains("t_item"));
    assert!(denied(&r, "SELECT sql FROM sqlite_schema").contains("sqlite_"));
    assert!(denied(&r, "SELECT * FROM sqlite_master").contains("sqlite_master"));
    assert!(denied(&r, "SELECT * FROM pragma_table_info('t_item')").contains("pragma"));
}

#[test]
fn the_statement_gate_refuses_anything_but_one_query() {
    let r = ew_sql();
    assert_eq!(r.query(""), Err(SqlError::Empty));
    assert_eq!(r.query("-- nothing\n/* at all */"), Err(SqlError::Empty));
    assert!(matches!(r.query("EXPLAIN SELECT 1"), Err(SqlError::NotAQuery(w)) if w == "EXPLAIN"));
    assert!(matches!(
        r.query("PRAGMA query_only = 0"),
        Err(SqlError::NotAQuery(_))
    ));
    assert!(matches!(
        r.query("INSERT INTO t_item VALUES ('a', 1)"),
        Err(SqlError::NotAQuery(_))
    ));
    assert!(matches!(r.query("ATTACH 'other.db' AS x"), Err(SqlError::NotAQuery(w)) if w == "ATTACH"));
    assert!(matches!(
        r.query("VACUUM INTO 'x.db'"),
        Err(SqlError::NotAQuery(_))
    ));
    assert_eq!(r.query("SELECT 1; SELECT 2"), Err(SqlError::MultipleStatements));
    // A trailing comment or semicolon is not a second statement.
    assert_eq!(texts(&r, "SELECT 1; -- done"), ["1"]);
    assert_eq!(texts(&r, "VALUES (1), (2)"), ["1", "2"]);
}

#[test]
fn vacuum_into_behind_a_query_creates_no_file() {
    let dir = ew_testdir::fresh_dir("ew-sqlview-vacuum");
    let target = dir.join("x.db");
    let target_sql = target.to_string_lossy().replace('\'', "''");
    let r = ew_sql();
    let tail = r.query(&format!("SELECT 1; VACUUM INTO '{target_sql}'"));
    assert!(
        matches!(tail, Err(SqlError::MultipleStatements | SqlError::Denied(_))),
        "{tail:?}"
    );
    let with = r.query(&format!(
        "WITH a AS (SELECT 1) SELECT * FROM a; VACUUM INTO '{target_sql}'"
    ));
    assert!(with.is_err());
    assert!(!target.exists(), "VACUUM INTO wrote a file");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn writes_ddl_attach_and_pragmas_are_denied() {
    let r = ew_sql();
    let write = r.query("WITH a AS (SELECT 1) INSERT INTO t_item SELECT 'z', 9 FROM a");
    assert!(
        matches!(write, Err(SqlError::Denied(_) | SqlError::NotReadOnly)),
        "{write:?}"
    );
    let write = r.query("WITH a AS (SELECT 1) DELETE FROM t_item");
    assert!(
        matches!(write, Err(SqlError::Denied(_) | SqlError::NotReadOnly)),
        "{write:?}"
    );
    let tail = r.query("SELECT 1; ATTACH 'other.db' AS x");
    assert!(
        matches!(tail, Err(SqlError::Denied(ref w)) if w == "ATTACH")
            || matches!(tail, Err(SqlError::MultipleStatements)),
        "{tail:?}"
    );
    assert_eq!(
        texts(&r, "SELECT count(*) FROM api_v1_item"),
        ["5"],
        "nothing changed"
    );
}

#[test]
fn functions_outside_the_allowlist_are_denied() {
    let r = ew_sql();
    assert_eq!(denied(&r, "SELECT random()"), "function random");
    assert_eq!(denied(&r, "SELECT hex(randomblob(4))"), "function randomblob");
    assert_eq!(
        denied(&r, "SELECT load_extension('x')"),
        "function load_extension"
    );
    assert!(denied(&r, "SELECT sqlite_version()").contains("sqlite_version"));
    assert!(denied(&r, "SELECT changes()").contains("changes"));
    // The allowlisted families work.
    assert_eq!(
        texts(
            &r,
            "SELECT upper('a'), abs(-2), round(2.5), json_array_length('[1,2]')"
        ),
        ["A|2|3.0|2"]
    );
    assert_eq!(
        texts(&r, "SELECT value FROM json_each('[3,4]') ORDER BY value"),
        ["3", "4"]
    );
    assert_eq!(
        texts(
            &r,
            "SELECT name, row_number() OVER (ORDER BY n, name) FROM api_v1_item ORDER BY 2 LIMIT 2"
        ),
        ["X1|1", "x1|2"]
    );
}

#[test]
fn date_functions_reject_the_clock() {
    let r = ew_sql();
    assert_eq!(
        texts(
            &r,
            "SELECT date('2025-03-03', '+1 day'), strftime('%Y', '2025-04-14'), julianday('2000-01-01')"
        ),
        ["2025-03-04|2025|2451544.5"]
    );
    for sql in [
        "SELECT date('now')",
        "SELECT datetime('NOW')",
        "SELECT date()",
        "SELECT unixepoch()",
        "SELECT strftime('%Y')",
        "SELECT strftime('%Y', 'now')",
        "SELECT datetime('2025-01-01', 'localtime')",
        "SELECT datetime('2025-01-01', 'utc')",
        "SELECT CURRENT_TIMESTAMP",
        "SELECT CURRENT_DATE",
        "SELECT CURRENT_TIME",
    ] {
        match r.query(sql) {
            Err(SqlError::Sql(msg)) => {
                assert!(msg.contains("clock") || msg.contains("time zone"), "{sql}: {msg}")
            }
            other => panic!("{sql}: {other:?}"),
        }
    }
}

#[test]
fn ew_natural_sorts_digit_runs_by_value() {
    let r = ew_sql();
    assert_eq!(
        texts(
            &r,
            "SELECT name FROM api_v1_item ORDER BY name COLLATE ew_natural"
        ),
        ["-X1031:2", "X1", "x1", "X2", "X10"]
    );
    assert_eq!(
        texts(
            &r,
            "SELECT column1 FROM (VALUES ('X10'), ('X2')) ORDER BY column1 COLLATE ew_natural"
        ),
        ["X2", "X10"]
    );
}

#[test]
fn ew_ml_reads_one_language_with_fallback() {
    use ew_derive::ew_mltext::{LangTag, MlText};
    let ml = MlText::from_langs([
        (LangTag::parse_lenient("en").unwrap(), "Pneumatics"),
        (LangTag::parse_lenient("nl-NL").unwrap(), "Pneumatiek"),
    ]);
    let json = serde_json::to_string(&ml).unwrap().replace('\'', "''");
    let r = ew_sql();
    assert_eq!(
        texts(&r, &format!("SELECT ew_ml('{json}', 'nl-NL')")),
        ["Pneumatiek"]
    );
    assert_eq!(
        texts(&r, &format!("SELECT ew_ml('{json}', 'en')")),
        ["Pneumatics"]
    );
    assert_eq!(
        texts(&r, &format!("SELECT ew_ml('{json}', 'fr')")),
        ["Pneumatics"]
    );
    assert_eq!(texts(&r, "SELECT ew_ml(NULL, 'en') IS NULL"), ["1"]);
    assert!(matches!(
        r.query("SELECT ew_ml('not json', 'en')"),
        Err(SqlError::Sql(_))
    ));
}

#[test]
fn a_recursive_cte_bomb_hits_the_deadline() {
    let r = reader(Profile::EW_SQL.with_deadline(Duration::from_millis(200)));
    let bomb = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c";
    assert_eq!(r.query(bomb), Err(SqlError::Deadline { ms: 200 }));
    // The reader stays usable after an interrupted statement.
    assert_eq!(texts(&r, "SELECT count(*) FROM api_v1_item"), ["5"]);
}

#[test]
fn result_caps_stop_large_results() {
    let rows = Profile {
        max_rows: 3,
        ..Profile::EW_SQL
    };
    let r = reader(rows);
    assert_eq!(
        r.query("SELECT name FROM api_v1_item"),
        Err(SqlError::RowLimit { rows: 3 })
    );
    assert_eq!(
        texts(&r, "SELECT name FROM api_v1_item ORDER BY n, name LIMIT 3").len(),
        3
    );
    let bytes = Profile {
        max_bytes: 100,
        ..Profile::EW_SQL
    };
    let r = reader(bytes);
    assert_eq!(
        r.query("SELECT printf('%.200c', 'x')"),
        Err(SqlError::ByteLimit { bytes: 100 })
    );
    // An unbounded result of a recursive CTE stops at the row cap of the profile.
    let r = reader(Profile {
        max_rows: 1_000,
        ..Profile::EW_SQL
    });
    assert_eq!(
        r.query("WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT x FROM c"),
        Err(SqlError::RowLimit { rows: 1_000 })
    );
}

#[test]
fn limits_bound_expression_depth_and_length() {
    let r = ew_sql();
    let deep = format!("SELECT {}1{}", "-(".repeat(300), ")".repeat(300));
    assert!(
        matches!(r.query(&deep), Err(SqlError::Sql(_))),
        "expression depth"
    );
    let long = format!("SELECT '{}'", "x".repeat(300 * 1024));
    assert!(matches!(r.query(&long), Err(SqlError::Sql(_))), "SQL length");
    let compound = format!("SELECT 1{}", " UNION ALL SELECT 1".repeat(60));
    assert!(
        matches!(r.query(&compound), Err(SqlError::Sql(_))),
        "compound select"
    );
    let big = r.query("SELECT length(zeroblob(20000000))");
    assert!(matches!(big, Err(SqlError::Sql(_))), "LENGTH limit: {big:?}");
}

#[test]
fn a_count_reads_the_flattened_view_and_nothing_else() {
    let r = ew_sql();
    // SQLite flattens the view and reads the table with an empty column name for count(*).
    assert_eq!(texts(&r, "SELECT count(*) FROM api_v1_item"), ["5"]);
    assert_eq!(
        texts(&r, "SELECT count(*) FROM (SELECT 1 FROM api_v1_item)"),
        ["5"]
    );
    // The same column-less read without the view is denied, also after a statement that used the view.
    assert!(denied(&r, "SELECT count(*) FROM t_item").contains("t_item"));
    assert!(denied(&r, "SELECT 1 FROM t_item").contains("t_item"));
    // A common table expression whose name merely looks like a view is not one.
    assert!(
        denied(
            &r,
            "WITH api_v1_evil AS (SELECT name FROM t_item) SELECT * FROM api_v1_evil"
        )
        .contains("t_item")
    );
    assert!(
        denied(
            &r,
            "WITH api_v1_evil AS (SELECT sql FROM sqlite_schema) SELECT * FROM api_v1_evil"
        )
        .contains("sqlite_")
    );
}

#[test]
fn a_cte_named_after_a_view_reads_only_that_views_table() {
    let r = ew_sql();
    assert_eq!(texts(&r, "SELECT code FROM api_v1_other"), ["a"]);
    // Cross-table: a CTE named after one view reading another view's table.
    assert!(
        denied(
            &r,
            "WITH api_v1_other AS (SELECT name FROM t_item) SELECT * FROM api_v1_other"
        )
        .contains("t_item")
    );
    assert!(
        denied(
            &r,
            "WITH api_v1_item AS (SELECT rowid, * FROM t_other) SELECT * FROM api_v1_item"
        )
        .contains("t_other")
    );
    assert!(denied(&r, "SELECT i.name FROM api_v1_item AS i JOIN t_other AS o ON 1").contains("t_other"));
}

#[test]
fn date_functions_reject_the_clock_in_blob_arguments() {
    let r = ew_sql();
    for sql in [
        "SELECT date(CAST('now' AS BLOB))",
        "SELECT unixepoch(x'6e6f77')",
        "SELECT datetime('2025-01-01', CAST('localtime' AS BLOB))",
        "SELECT datetime('2025-01-01', CAST('UTC' AS BLOB))",
        "SELECT strftime('%Y', x'4e4f57')",
    ] {
        match r.query(sql) {
            Err(SqlError::Sql(msg)) => {
                assert!(msg.contains("clock") || msg.contains("time zone"), "{sql}: {msg}")
            }
            other => panic!("{sql}: {other:?}"),
        }
    }
    // A blob holding a date is still a date.
    assert_eq!(
        texts(&r, "SELECT date(CAST('2025-03-03' AS BLOB))"),
        ["2025-03-03"]
    );
}

#[test]
fn a_huge_deadline_is_capped_and_never_overflows_the_clock() {
    let capped = Profile::EW_SQL.with_deadline(Duration::from_secs(u64::MAX));
    assert_eq!(capped.deadline, Profile::MAX_DEADLINE);
    let r = reader(capped);
    assert_eq!(texts(&r, "SELECT count(*) FROM api_v1_item"), ["5"]);
    // A hand-built profile beyond the cap runs too.
    let r = reader(Profile {
        deadline: Duration::MAX,
        ..Profile::EW_SQL
    });
    assert_eq!(texts(&r, "SELECT count(*) FROM api_v1_item"), ["5"]);
}

#[test]
fn the_profiles_carry_the_documented_limits() {
    assert_eq!(Profile::EW_SQL.deadline, Duration::from_secs(60));
    assert_eq!(Profile::EW_SQL.max_rows, 1_000_000);
    assert_eq!(Profile::EW_SQL.max_bytes, 256 * 1024 * 1024);
    assert_eq!(Profile::REPORT.deadline, Duration::from_secs(2));
    assert_eq!(Profile::REPORT.max_rows, 100_000);
    assert_eq!(Profile::RULE.deadline, Duration::from_secs(10));
    assert_eq!(Profile::MCP.deadline, Duration::from_secs(5));
    assert_eq!(Profile::MCP.max_rows, 1_000);
    assert_eq!(Profile::MCP.max_bytes, 1024 * 1024);
}
