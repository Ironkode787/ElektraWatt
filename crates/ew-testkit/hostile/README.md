<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# The hostile corpus

Files made to attack a parser of external bytes (WP-S6.04 D10; doc 18 §3.12; NFR-SEC-02, NFR-SEC-06). They are the
project's own work, CC0-1.0 (the REUSE annotations hold the licence, since a data file carries no header). Each file
stands in the folder of its class and is named after what it tries; this index names its parser and the verdict
expected. A test asserts each verdict whole; the files of `sql/` are also the seed corpus of the fuzz target
`sql_sandbox` (`fuzz/corpus/sql_sandbox/`, the same bytes).

## `sql/`: hostile SQL text (FT-223)

Parser: the SQL sandbox, `ew_sqlview::Reader::query`, under `ew sql`'s profile with a 200 ms deadline, on the small
image of the sandbox tests (`t_item` behind `api_v1_item`, `t_other` behind `api_v1_other`). The test is
`the_hostile_sql_corpus_is_refused_file_by_file` in `crates/ew-sqlview/src/sandbox_tests.rs`.

| File | Tries | Verdict |
|---|---|---|
| `attach.sql` | to attach another database | Refused by the statement gate: `ATTACH` is not a query |
| `clock-in-a-blob.sql` | to read the clock through `date()` given `now` as a BLOB | Refused by the clock-free date function |
| `cte-named-after-a-view.sql` | to read an internal table through a CTE named after a view | Denied by the authorizer: a read of `t_other` |
| `deep-expression.sql` | an expression nested 300 deep | Stopped by `SQLITE_LIMIT_EXPR_DEPTH` (200) |
| `load-extension.sql` | to load a native extension | Denied by the authorizer: the function `load_extension` |
| `pragma-read.sql` | to read the schema through a table-valued `PRAGMA` | Denied by the authorizer: a read of `pragma_table_info` |
| `recursive-cte-count.sql` | an endless recursive CTE | Stopped by the deadline |
| `recursive-cte-doubling.sql` | a recursive CTE that doubles a string | Stopped by `SQLITE_LIMIT_LENGTH` (16 MiB) |
| `two-statements.sql` | two statements in one call | Refused by the statement gate |
| `vacuum-into-behind-a-query.sql` | `VACUUM INTO` after a query, to write a file | Refused by the statement gate; no file is written |
| `write-behind-a-cte.sql` | a `DELETE` behind a `WITH` | Denied by the authorizer: a `DELETE` |
