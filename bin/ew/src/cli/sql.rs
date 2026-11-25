// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew sql` and `ew sql dump`: a statement over the projection, under its panic guard.

use super::*;

/// `sql` is a path of its own (the query) with the paths `sql dump` and `sql schema` below it.
impl Leaf for SqlCli {
    const EXITS: &'static [Exit] = &[OK, exit(2, "usage, open and SQL errors, denials and limits")];
    const RESULT: Option<&'static str> = Some(results::SQL);

    fn check(&self) -> Result<(), clap::Error> {
        match &self.sub {
            Some(sub) => sub.check(),
            None => {
                self.project_and_query()?;
                self.csv_options(self.result_format()?).map(drop)
            }
        }
    }

    fn below(cmd: &clap::Command, path: &mut Vec<String>, out: &mut Vec<crate::catalogue::Entry>) {
        SqlSub::entries(cmd, path, out);
    }
}

impl Leaf for SqlSchemaCli {
    const EXITS: &'static [Exit] = &[OK];
}

/// `ew sql <select>`: the statement over the projection, its result as CSV, JSON or a workbook.
pub(super) fn run_sql(a: &SqlCli, io: &mut Io<'_>) -> i32 {
    let line = a.project_and_query().and_then(|(project, query)| {
        let format = a.result_format()?;
        Ok((project, query, format, a.csv_options(format)?))
    });
    let (project, query, format, csv) = match line {
        Ok(line) => line,
        Err(e) => return usage(&e, io),
    };
    let sql = match &query {
        SqlSource::Text(t) => t.clone(),
        SqlSource::File(f) => match std::fs::read_to_string(f) {
            Ok(t) => t,
            Err(e) => {
                io.error(format_args!("{}: {e}", f.display()));
                return 2;
            }
        },
    };
    let (projection, session) = match open_projection_and_session(project.as_ref(), io) {
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
        debug_panic("sql", io.env);
        let set = projection.reader(profile).and_then(|r| r.query(&sql))?;
        debug_panic("sql-write", io.env);
        let text = match format {
            SqlFormat::Csv if csv.preset == Preset::Ew => Some(ew_api::output::restyle(
                &set.to_csv(),
                csv.writes_bom(),
                csv.writes_line_end(),
            )),
            SqlFormat::Csv => Some(ew_api::output::table_csv(&ew_api::output::sql_table(&set), &csv)),
            SqlFormat::Json => Some(format!("{:#}\n", set.to_json())),
            SqlFormat::Xlsx => None,
        };
        Ok((set, text))
    }) {
        Ok(s) => s,
        Err(line) => {
            errln!(io, "{line}");
            return 2;
        }
    };
    let Some(text) = text else {
        return write_sql_xlsx(a.out.as_ref(), &session, &set, io);
    };
    match &a.out {
        Some(path) => match write_report(path, &text) {
            Ok(()) => outln!(io, "result written to {}", path.display()),
            Err(e) => {
                io.error(format_args!("{}: {e}", path.display()));
                return 2;
            }
        },
        None => {
            out!(io, "{text}");
            errln!(io, "{} rows", set.rows.len());
        }
    }
    0
}

/// `ew sql`'s refusal when the statement panicked.
pub(super) const QUERY_PANICKED: &str = "the query stopped on an internal error; nothing was changed";

/// Runs `ew sql`'s statement, `run`, inside `catch_unwind` (NFR-REL-12): its result, or the line to print
/// (`error: ` and the SQL error, or [`QUERY_PANICKED`] when it panicked). The query reads a projection image only, so a
/// panic changes nothing.
pub(super) fn guarded_query<T>(run: impl FnOnce() -> Result<T, ew_api::sql::SqlError>) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Ok(t)) => Ok(t),
        Ok(Err(e)) => Err(error_line(e)),
        Err(_) => Err(error_line(QUERY_PANICKED)),
    }
}

/// `ew sql --format xlsx --out <file>`: the result as the one sheet `sql` of a workbook, written atomically.
pub(super) fn write_sql_xlsx(
    out: Option<&PathBuf>,
    session: &ew_api::Session,
    set: &ew_api::sql::ResultSet,
    io: &mut Io<'_>,
) -> i32 {
    let Some(path) = out else {
        io.error(XLSX_NEEDS_OUT);
        return 2;
    };
    let date = match source_date(io.env) {
        Ok(d) => d,
        Err(e) => {
            io.error(e);
            return 2;
        }
    };
    let meta = session.workbook_meta(date);
    // The workbook is written from the result inside the statement's guard.
    let bytes = match guarded_query(|| {
        debug_panic("sql-write", io.env);
        Ok(ew_api::output::xlsx(&[ew_api::output::sql_table(set)], &meta))
    }) {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            io.error(e);
            return 2;
        }
        Err(line) => {
            errln!(io, "{line}");
            return 2;
        }
    };
    if let Err(e) = write_atomically(path, &bytes) {
        io.error(format_args!("{}: {e}", path.display()));
        return 2;
    }
    outln!(io, "result written to {}", path.display());
    0
}

/// The refusal of `--format xlsx` without `--out`.
pub const XLSX_NEEDS_OUT: &str = "`--format xlsx` writes a file: give `--out <file.xlsx>`";

impl Leaf for SqlDumpCli {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
    const RESULT: Option<&'static str> = Some(results::SqlDumpResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.project_and_out().map(drop)
    }
}

pub(super) fn run_sql_dump(a: &SqlDumpCli, io: &mut Io<'_>) -> i32 {
    let (project, out) = match a.project_and_out() {
        Ok(line) => line,
        Err(e) => return usage(&e, io),
    };
    let projection = match open_projection(project.as_ref(), io) {
        Ok(p) => p,
        Err(code) => return code,
    };
    match std::fs::write(&out, projection.image()) {
        Ok(()) if a.json => {
            out!(
                io,
                "{}",
                results::to_text(&results::SqlDumpResult {
                    schema: results::SqlDumpResult::SCHEMA.to_owned(),
                    out: out.display().to_string(),
                })
            );
            0
        }
        Ok(()) => {
            outln!(io, "projection written to {}", out.display());
            0
        }
        Err(e) => {
            io.error(format_args!("{}: {e}", out.display()));
            2
        }
    }
}

#[cfg(test)]
mod tests {
    //! `ew sql`'s statement under its panic guard, given a panicking closure, on a copy of the sample project in a
    //! fresh directory.
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

    /// S1 whole, when the statement panics after it ran on the projection of a copy of the sample; the copy's
    /// files and the session's model are unchanged; an SQL error keeps its own line.
    #[test]
    fn a_panicking_statement_gives_s1_and_changes_nothing() {
        let root = ew_testdir::Tmp::new("ew-cli-guard");
        let dir = root.sample("sample.ew");
        let files = |d: &Path| {
            let mut out = std::collections::BTreeMap::new();
            tree(d, d, &mut out);
            out
        };
        let before = files(&dir);
        let env = crate::cli::io::Env::new(dir.clone());
        let (projection, session) =
            crate::cli::io::captured("", &env, |io| open_projection_and_session(Some(&dir), io))
                .0
                .unwrap();
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
    }
}
