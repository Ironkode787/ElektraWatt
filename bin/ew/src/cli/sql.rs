// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew sql` and `ew sql dump`: a statement over the projection, under its panic guard.

use super::*;

pub(super) fn run_sql(a: &SqlArgs) -> i32 {
    let sql = match &a.query {
        SqlSource::Text(t) => t.clone(),
        SqlSource::File(f) => match std::fs::read_to_string(f) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: {}: {e}", f.display());
                return 2;
            }
        },
    };
    let (projection, session) = match open_projection_and_session(a.project.as_ref()) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let mut profile = ew_api::sql::Profile::EW_SQL;
    if let Some(n) = a.timeout {
        profile = profile.with_deadline(std::time::Duration::from_secs(n));
    }
    // The result's writing, which a hostile SELECT drives as much as the statement, runs inside the guard
    // too (the workbook's in `write_sql_xlsx`).
    let (set, text) = match guarded_query(|| {
        debug_panic("sql");
        let set = projection.reader(profile).and_then(|r| r.query(&sql))?;
        debug_panic("sql-write");
        let text = match a.format {
            SqlFormat::Csv if a.csv.preset == Preset::Ew => Some(ew_api::output::restyle(
                &set.to_csv(),
                a.csv.writes_bom(),
                a.csv.writes_line_end(),
            )),
            SqlFormat::Csv => Some(ew_api::output::table_csv(
                &ew_api::output::sql_table(&set),
                &a.csv,
            )),
            SqlFormat::Json => Some(format!("{:#}\n", set.to_json())),
            SqlFormat::Xlsx => None,
        };
        Ok((set, text))
    }) {
        Ok(s) => s,
        Err(line) => {
            eprintln!("{line}");
            return 2;
        }
    };
    let Some(text) = text else {
        return write_sql_xlsx(a, &session, &set);
    };
    match &a.out {
        Some(path) => match write_report(path, &text) {
            Ok(()) => println!("result written to {}", path.display()),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
        None => {
            print!("{text}");
            eprintln!("{} rows", set.rows.len());
        }
    }
    0
}

/// S1 of WP-S6.04 (D15): `ew sql`'s line when the statement panicked.
pub(super) const QUERY_PANICKED: &str = "error: the query stopped on an internal error; nothing was changed";

/// Runs `ew sql`'s statement, `run`, inside `catch_unwind` (NFR-REL-12): its result, or the line to print
/// (`error: <the SQL error>`, or [`QUERY_PANICKED`] when it panicked). The query reads a projection image only, so a
/// panic changes nothing.
pub(super) fn guarded_query<T>(run: impl FnOnce() -> Result<T, ew_api::sql::SqlError>) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Ok(t)) => Ok(t),
        Ok(Err(e)) => Err(format!("error: {e}")),
        Err(_) => Err(QUERY_PANICKED.to_owned()),
    }
}

/// `ew sql --format xlsx --out <file>`: the result as the one sheet `sql` of a workbook, written atomically.
pub(super) fn write_sql_xlsx(a: &SqlArgs, session: &ew_api::Session, set: &ew_api::sql::ResultSet) -> i32 {
    let Some(path) = &a.out else {
        eprintln!("error: {XLSX_NEEDS_OUT}");
        return 2;
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let meta = session.workbook_meta(date);
    // The workbook is written from the result inside the statement's guard.
    let bytes = match guarded_query(|| {
        debug_panic("sql-write");
        Ok(ew_api::output::xlsx(&[ew_api::output::sql_table(set)], &meta))
    }) {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            eprintln!("error: {e}");
            return 2;
        }
        Err(line) => {
            eprintln!("{line}");
            return 2;
        }
    };
    if let Err(e) = write_atomically(path, &bytes) {
        eprintln!("error: {}: {e}", path.display());
        return 2;
    }
    println!("result written to {}", path.display());
    0
}

/// The refusal of `--format xlsx` without `--out`.
pub const XLSX_NEEDS_OUT: &str = "`--format xlsx` writes a file: give `--out <file.xlsx>`";

pub(super) fn run_sql_dump(a: &SqlDumpArgs) -> i32 {
    let projection = match open_projection(a.project.as_ref()) {
        Ok(p) => p,
        Err(code) => return code,
    };
    match std::fs::write(&a.out, projection.image()) {
        Ok(()) if a.json => {
            print!(
                "{}",
                results::to_text(&results::SqlDumpResult {
                    schema: results::SQL_DUMP.to_owned(),
                    out: a.out.display().to_string(),
                })
            );
            0
        }
        Ok(()) => {
            println!("projection written to {}", a.out.display());
            0
        }
        Err(e) => {
            eprintln!("error: {}: {e}", a.out.display());
            2
        }
    }
}

#[cfg(test)]
mod tests {
    //! `ew sql`'s statement under its panic guard, given a panicking closure, on a copy of
    //! Golden A in a fresh directory; nothing under `golden/` is opened for writing.
    use super::*;

    /// Every file below `dir` with its bytes, by path relative to `base`.
    fn tree(dir: &Path, base: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                tree(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }

    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let p = e.unwrap().path();
            let target = to.join(p.file_name().unwrap());
            if p.is_dir() {
                copy(&p, &target);
            } else {
                std::fs::copy(&p, &target).unwrap();
            }
        }
    }

    /// `ew import table`'s refusal lines, each whole: `error: ` and the refusal, its first letter in lower
    /// case (S2 for a read that panicked).
    #[test]
    fn an_import_table_refusal_is_one_lower_case_error_line() {
        use ew_api::output::TableImportError as E;
        assert_eq!(
            import_table_error_line(&E::Panicked("pages.csv".into())),
            "error: the table import stopped on an internal error while reading pages.csv; nothing was changed."
        );
        assert_eq!(
            import_table_error_line(&E::CsvNoTable),
            "error: a CSV names no table: give `--table <key>`"
        );
    }

    /// S1 whole, when the statement panics after it ran on the projection of a copy of Golden A; the copy's files
    /// and the session's model are unchanged; an SQL error keeps its own line.
    #[test]
    fn a_panicking_statement_gives_s1_and_changes_nothing() {
        let root = ew_testdir::fresh_dir("ew-cli-guard");
        let dir = root.join("J-E-2416.ew");
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew"),
            &dir,
        );
        let files = |d: &Path| {
            let mut out = std::collections::BTreeMap::new();
            tree(d, d, &mut out);
            out
        };
        let before = files(&dir);
        let (projection, session) = open_projection_and_session(Some(&dir)).unwrap();
        let model = session.world().model.clone();
        let r: Result<(), String> = guarded_query(|| {
            let set = projection
                .reader(ew_api::sql::Profile::EW_SQL)
                .and_then(|r| r.query("SELECT count(*) FROM api_v1_terminal"))?;
            assert_eq!(set.rows.len(), 1);
            panic!("a statement that panics")
        });
        assert_eq!(
            r,
            Err("error: the query stopped on an internal error; nothing was changed".to_owned())
        );
        let r = guarded_query(|| {
            projection
                .reader(ew_api::sql::Profile::EW_SQL)
                .and_then(|r| r.query("DELETE FROM api_v1_terminal"))
        });
        assert_eq!(
            r.map(|_| ()),
            Err("error: statement denied by the projection sandbox (DELETE): only SELECT, WITH or VALUES may run"
                .to_owned())
        );
        assert!(session.world().model.content_eq(&model), "the model changed");
        drop((projection, session));
        assert!(files(&dir) == before, "the copy's files changed");
        let _ = std::fs::remove_dir_all(&root);
    }
}
