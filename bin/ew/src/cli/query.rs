// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew query` and `ew queries`: the public queries of the session, with `--explain`,
//! and the query registry.

use ew_api::cli_results as results;

use super::error;
use super::tree::{Leaf, QueriesCli, QueryCli, usage};
use super::{open_reader, reader_dir};
use crate::catalogue::{Exit, OK, exit};

impl Leaf for QueryCli {
    const EXITS: &'static [Exit] = &[
        exit(0, "answered"),
        exit(
            2,
            "usage, an unknown query, open errors, a key that names no record or one of another kind",
        ),
    ];
    const RESULT: Option<&'static str> = Some(results::QueryResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.project_name_key().map(drop)
    }
}

/// `ew query` (D11, D12, FR-EXT-005): opens the project as a reader (no lock) and prints the query's value, with
/// `--explain` its policy, dependencies and provenance. Exit 0 when answered, 2 on usage, an unknown query, open
/// errors and a key that names no record or a record of another kind.
pub(super) fn run_query(a: &QueryCli) -> i32 {
    let (project, name, key) = match a.project_name_key() {
        Ok(line) => line,
        Err(e) => return usage(&e),
    };
    // An unknown name is refused before the project is opened.
    if ew_api::queries::query(name).is_none() {
        error(format_args!(
            "{}",
            ew_api::queries::QueryError::Unknown(name.to_owned())
        ));
        return 2;
    }
    let Some(dir) = reader_dir(project.as_ref()) else {
        return 2;
    };
    let session = match open_reader(&dir, "query") {
        Ok((session, _)) => session,
        Err(e) => {
            error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let answer = match session.run_query(name, key, a.explain) {
        Ok(answer) => answer,
        Err(e) => {
            error(e);
            return 2;
        }
    };
    if a.json {
        print!(
            "{}",
            results::to_text(&results::QueryResult {
                schema: results::QueryResult::SCHEMA.to_owned(),
                query: answer.query.to_owned(),
                key: answer.key.clone(),
                rev: answer.rev,
                output: answer.output.clone(),
                explain: answer.explain.clone(),
            })
        );
    } else {
        print!("{}", answer.text());
    }
    0
}

impl Leaf for QueriesCli {
    const EXITS: &'static [Exit] = &[OK];
    const RESULT: Option<&'static str> = Some(results::QueriesResult::SCHEMA);
}

/// `ew queries` (D11, D13): the query registry, with `--json` the text of `crates/ew-api/api/queries.json`.
pub(super) fn run_queries(json: bool) -> i32 {
    let list = results::queries();
    if json {
        print!("{}", results::to_text(&list));
        return 0;
    }
    for q in &list.queries {
        println!("{}\t{}\t{}", q.name, q.key, q.description);
    }
    0
}
