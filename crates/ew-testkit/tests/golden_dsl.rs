// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Build path (b) of Golden A (doc 20 §3.2, WP-S6.01): the builder DSL constructs the golden project
//! through commands only, from the shell, and the acceptance SQL over it returns the expected CSVs.

use std::path::{Path, PathBuf};

use ew_api::{AnyCommand, Session, SessionOptions};
use ew_commands::Cost;
use ew_testkit::golden::dsl::Done;
use ew_testkit::golden::{self, Built};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs one command on `session`: a bulk command as preview and commit.
fn run(session: &mut Session, name: &str, params: serde_json::Value) -> Result<Done, String> {
    let cmd = AnyCommand {
        name: name.to_owned(),
        params,
    };
    let bulk = session
        .registry()
        .get(name)
        .is_some_and(|e| e.meta().cost == Cost::Bulk);
    let receipt = if bulk {
        session.preview(&cmd).and_then(|p| session.commit(p))
    } else {
        session.execute(&cmd)
    };
    receipt
        .map(|r| Done {
            primary: r.outcome.primary,
            created: r.created,
        })
        .map_err(|e| format!("{e:?}"))
}

fn built(seed: u64) -> (Session, Built) {
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    let shell = golden::shell(&batch);
    let mut session = Session::from_records(
        shell.project,
        [],
        shell.lib,
        SessionOptions {
            ids: Some(Box::new(ew_testkit::synth::SynthSource::new(seed))),
            memo: ew_api::MemoMode::Off,
            ..SessionOptions::default()
        },
    )
    .expect("the shell is a valid model");
    let built = golden::build(&batch, &mut |name: &str, params| run(&mut session, name, params))
        .unwrap_or_else(|e| panic!("path (b): {e}"));
    (session, built)
}

#[test]
fn path_b_returns_the_expected_csvs() {
    let (session, built) = built(1);
    eprintln!("commands: {:?}", built.by_command);
    eprintln!("unwritten: {}", built.unwritten_summary());
    let checks = session.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Release);
    let projection = session
        .sql_projection(Some(&checks))
        .expect("the projection builds");
    let mut failures = Vec::new();
    for name in [
        "terminals_1X1",
        "terminals_X1031",
        "terminals_X1032",
        "wires",
        "potentials",
        "cables",
    ] {
        let sql =
            std::fs::read_to_string(repo().join(format!("golden/J-E-2416/acceptance/{name}.sql"))).unwrap();
        let got = projection
            .reader(ew_api::sql::Profile::EW_SQL)
            .and_then(|r| r.query(&sql))
            .map(|set| set.to_csv())
            .unwrap_or_else(|e| format!("error: {e}"));
        let want =
            std::fs::read_to_string(repo().join(format!("golden/J-E-2416/expected/csv/{name}.csv"))).unwrap();
        if got != want {
            eprintln!("=== {name} differs\n--- got\n{got}\n--- want\n{want}");
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
