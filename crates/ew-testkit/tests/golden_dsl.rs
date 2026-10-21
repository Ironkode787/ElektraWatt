// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Build path (b) of Golden A (doc 20 §3.2, WP-S6.01): the builder DSL constructs the golden project
//! through commands only, from the shell; the acceptance SQL over it is compared with the expected CSVs.
//!
//! The acceptance runner (`crates/ew-api/tests/acceptance.rs`, AC-15) reports the comparison; these tests
//! pin what the builder writes and leaves out, its determinism, the replay of its command log and the
//! undo of every step.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::{AnyCommand, CmdLog, HistoryLimits, Session, SessionOptions};
use ew_commands::Cost;
use ew_testkit::golden::dsl::Done;
use ew_testkit::golden::{self, Built};

const SQL: [&str; 6] = [
    "terminals_1X1",
    "terminals_X1031",
    "terminals_X1032",
    "wires",
    "potentials",
    "cables",
];

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

/// A session over the shell of path (a)'s batch, IDs from `seed`, without the memo.
fn shell_session(seed: u64) -> Session {
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    let shell = golden::shell(&batch);
    Session::from_records(
        shell.project,
        [],
        shell.lib,
        SessionOptions {
            ids: Some(Box::new(ew_testkit::synth::SynthSource::new(seed))),
            memo: ew_api::MemoMode::Off,
            history: HistoryLimits {
                max_steps: 10_000,
                ..HistoryLimits::default()
            },
            ..SessionOptions::default()
        },
    )
    .expect("the shell is a valid model")
}

fn built(seed: u64) -> (Session, Built) {
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    let mut session = shell_session(seed);
    let built = golden::build(&batch, &mut |name: &str, params| run(&mut session, name, params))
        .unwrap_or_else(|e| panic!("path (b): {e}"));
    (session, built)
}

/// The result of every acceptance SQL file over `session`, as CSV.
fn sql_results(session: &Session) -> BTreeMap<&'static str, String> {
    let checks = session.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Release);
    let projection = session
        .sql_projection(Some(&checks))
        .expect("the projection builds");
    SQL.iter()
        .map(|name| {
            let sql = std::fs::read_to_string(repo().join(format!("golden/J-E-2416/acceptance/{name}.sql")))
                .expect("the SQL file reads");
            let csv = projection
                .reader(ew_api::sql::Profile::EW_SQL)
                .and_then(|r| r.query(&sql))
                .map(|set| set.to_csv())
                .unwrap_or_else(|e| format!("error: {e}"));
            (*name, csv)
        })
        .collect()
}

fn expected(name: &str) -> String {
    std::fs::read_to_string(repo().join(format!("golden/J-E-2416/expected/csv/{name}.csv")))
        .expect("expected CSV")
}

/// AC-15 path (b): the acceptance SQL over the command-built project returns the expected CSVs, and no
/// representation of the tables is refused (the page-type rule as amended by the owner ruling on ID-55:
/// the cable markers are graphic on folios 106 and 3, the CPX black boxes multi-line on 203 … 206).
#[test]
fn path_b_returns_the_expected_csvs() {
    let (session, built) = built(1);
    for (name, csv) in sql_results(&session) {
        assert_eq!(csv, expected(name), "acceptance/{name}.sql over path (b)");
    }
    assert!(
        !built
            .unwritten
            .keys()
            .any(|k| k.starts_with("representation (refused")),
        "{}",
        built.unwritten_summary()
    );
}

/// What path (b) issues and what it cannot write yet (no command): pinned, so that a new command or a
/// table change shows here.
#[test]
fn path_b_issues_these_commands_and_leaves_out_these_records() {
    let (_, built) = built(1);
    let commands: Vec<String> = built
        .by_command
        .iter()
        .map(|(k, n)| format!("{k}: {n}"))
        .collect();
    assert_eq!(
        commands.join(", "),
        "bond.create: 1, box.black: 11, cable.create: 3, connection.draw: 87, device.create: 36, \
         device.place: 48, external_ref.create: 4, fields.set: 125, ip.place: 48, junction.edit: 6, \
         page.create: 13, potential.define: 19, project.settings.set: 3, report_block.create: 15, \
         section.create: 3, structure.create_node: 4, terminal.add: 4"
    );
    assert_eq!(
        built.unwritten_summary(),
        "channel: 56, device (child tag scheme: valve-terminal assembly, FT-169): 29, device.assembly: 1, \
         device.parent: 11, device.plc_module: 7, device.plc_station: 1, device.position: 11, \
         device_link: 8, function (of a child-scheme device): 28, plc_config_project: 1, \
         project.supply.origin.source: 1, representation (of a child-scheme device): 28, \
         revision or other kind without a schema type: 2"
    );
}

/// The same seed builds the same model; another seed mints other IDs and gives the same SQL results.
#[test]
fn path_b_is_deterministic_and_compares_by_content() {
    let (a, built_a) = built(1);
    let (b, built_b) = built(1);
    assert!(
        a.world().model.content_eq(&b.world().model),
        "same seed, same model"
    );
    assert_eq!(built_a.script, built_b.script);
    let (c, _) = built(2);
    assert!(
        !a.world().model.content_eq(&c.world().model),
        "another seed mints other IDs"
    );
    assert_eq!(
        sql_results(&a),
        sql_results(&c),
        "the SQL results do not depend on the IDs"
    );
}

/// The command log of path (b) replays with `Session::run_script` (`ew apply`) on the shell with the same
/// ID source to the same model; undoing every step returns to the shell.
#[test]
fn the_command_log_replays_and_every_step_undoes() {
    let (mut session, built) = built(1);
    let log = CmdLog::parse(&built.command_log("Golden A, path (b)")).expect("the log parses");
    assert_eq!(log.len(), built.script.len());
    let mut replay = shell_session(1);
    replay
        .run_script(&log, "golden-a-path-b.ewcmd")
        .unwrap_or_else(|e| panic!("replay: {e}"));
    assert!(
        replay.world().model.content_eq(&session.world().model),
        "the replay builds the same model"
    );
    let mut steps = 0;
    while session.undo().is_ok() {
        steps += 1;
    }
    // one undo step per command that changed the model: two of the commands leave the model as it was
    // (a value the record already had), so they leave no step
    assert_eq!((steps, built.script.len()), (428, 430), "undo steps, commands");
    assert!(
        session.world().model.content_eq(&shell_session(1).world().model),
        "undoing every step returns to the shell"
    );
}
