// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew query` and `ew queries` (WP-S1.22 part 4, D11 and D12): the public queries of the session, with `--explain`,
//! and the query registry.

use std::path::PathBuf;

use ew_api::cli_results as results;

use super::project_dir;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The arguments of `ew query`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QueryArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The query's name (`device_tag`).
    pub name: String,
    /// The key (`tag:-101F1`, `-`).
    pub key: String,
    /// Add the explanation (`--explain`).
    pub explain: bool,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// `ew query` (D11, D12, FR-EXT-005): opens the project as a reader (no lock) and prints the query's value, with
/// `--explain` its policy, dependencies and provenance. Exit 0 when answered, 2 on usage, an unknown query, open
/// errors and a key that names no record or a record of another kind.
pub(super) fn run_query(a: &QueryArgs) -> i32 {
    // An unknown name is refused before the project is opened.
    if ew_api::queries::query(&a.name).is_none() {
        eprintln!("error: {}", ew_api::queries::QueryError::Unknown(a.name.clone()));
        return 2;
    }
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let session = match ew_api::Session::open_reader(&dir, VERSION, "query") {
        Ok((session, _)) => session,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let answer = match session.run_query(&a.name, &a.key, a.explain) {
        Ok(answer) => answer,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if a.json {
        print!(
            "{}",
            results::to_text(&results::QueryResult {
                schema: results::QUERY.to_owned(),
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
