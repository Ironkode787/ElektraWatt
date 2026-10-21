// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The SQL projection over the golden project (WP-S1.10; ADR-0017, doc 17 §6): every `api_v1_*` view is
//! readable through the sandbox with the columns of `ew_sqlview::schema`; the acceptance SQL of
//! `golden/J-E-2416/acceptance/` returns `expected/csv/`; the acceptance criteria of FR-EXT-006 and
//! FR-EXT-007; the row counts; a deterministic image.
//!
//! The acceptance runner (`tests/acceptance.rs`) evaluates the same SQL/CSV pairs as the "SQL half" of AC-08
//! … AC-11; this file gives the row-by-row difference when one fails.

#![allow(dead_code)]

mod golden_common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::sql::{Profile, Projection, SqlError, schema};

fn projection() -> Projection {
    let batch = golden_common::batch();
    let session = golden_common::session_with_profiles(&batch);
    let report = session.check(&PRE_DELIVERY, CheckMode::Release);
    session
        .sql_projection(Some(&report))
        .expect("the projection builds")
}

/// The acceptance SQL/CSV pairs (doc 20 §3.1): `acceptance/<name>.sql` returns `expected/csv/<name>.csv`.
const PAIRS: [&str; 6] = [
    "terminals_1X1",
    "terminals_X1031",
    "terminals_X1032",
    "cables",
    "wires",
    "potentials",
];

fn golden(path: &str) -> String {
    let p = golden_common::repo().join("golden/J-E-2416").join(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The lines of `got` that differ from `want`, numbered, for the failure message.
fn diff(want: &str, got: &str) -> String {
    let (w, g): (Vec<&str>, Vec<&str>) = (want.lines().collect(), got.lines().collect());
    let mut out = Vec::new();
    for i in 0..w.len().max(g.len()) {
        let (a, b) = (
            w.get(i).copied().unwrap_or("<none>"),
            g.get(i).copied().unwrap_or("<none>"),
        );
        if a != b {
            out.push(format!("line {}:\n  want {a}\n  got  {b}", i + 1));
        }
    }
    out.join("\n")
}

#[test]
fn the_acceptance_sql_returns_the_expected_csv() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let mut failures = Vec::new();
    for name in PAIRS {
        let sql = golden(&format!("acceptance/{name}.sql"));
        let want = golden(&format!("expected/csv/{name}.csv"));
        match reader.query(&sql) {
            Ok(set) => {
                let got = set.to_csv();
                if got != want {
                    failures.push(format!("{name}:\n{}", diff(&want, &got)));
                }
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn every_view_has_its_documented_columns_and_the_golden_counts() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    for v in schema::VIEWS {
        let set = reader.query(&format!("SELECT * FROM {}", v.name)).unwrap();
        let names: Vec<&str> = v.columns.iter().map(|c| c.name).collect();
        assert_eq!(set.columns, names, "{}", v.name);
    }
    let count = |view: &str| {
        reader
            .query(&format!("SELECT count(*) FROM {view}"))
            .unwrap()
            .rows[0][0]
            .text()
    };
    // Doc 17 §6.1 "Golden rows" and doc 20: 28 terminals, 3 cables, 13 conductors, 44 wires and 6 bridges,
    // the 27 rows of folio 901, one bond (B1). 18 of the 19 potentials of §7.3 (the DP bus potential needs
    // the Bus connections, FT-111) and 34 of the 36 pages of §5 (908.1 and 908.2 need PLC channel records):
    // the M1 gap list of the acceptance report.
    assert_eq!(count("api_v1_terminal"), "28");
    assert_eq!(count("api_v1_cable"), "3");
    assert_eq!(count("api_v1_conductor"), "13");
    assert_eq!(count("api_v1_wire"), "50");
    assert_eq!(
        reader
            .query("SELECT count(*) FROM api_v1_wire WHERE kind = 'bridge'")
            .unwrap()
            .rows[0][0]
            .text(),
        "6"
    );
    assert_eq!(count("api_v1_bom_component"), "27");
    assert_eq!(count("api_v1_bond"), "1");
    assert_eq!(count("api_v1_potential"), "18");
    assert_eq!(count("api_v1_page"), "34");
    assert_eq!(count("api_v1_meta"), "1");
    // The counts the projection reports equal the views'.
    for (view, n) in p.counts() {
        assert_eq!(count(view), n.to_string(), "{view}");
    }
}

#[test]
fn fr_ext_006_counts_terminals_and_denies_attach() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader.query("SELECT count(*) FROM api_v1_terminal").unwrap();
    assert_eq!(set.rows[0][0].text(), "28", "1X1: 3, -X1031: 20, -X1032: 5");
    assert!(matches!(
        reader.query("ATTACH 'other.db' AS x"),
        Err(SqlError::NotAQuery(_))
    ));
    assert!(matches!(
        reader.query("SELECT 1; ATTACH 'other.db' AS x"),
        Err(SqlError::Denied(_) | SqlError::MultipleStatements)
    ));
}

#[test]
fn fr_ext_007_sorts_designations_naturally_and_reads_multilingual_text() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader
        .query(
            "SELECT designation FROM api_v1_terminal WHERE strip_tag = '-X1031' \
             ORDER BY designation COLLATE ew_natural",
        )
        .unwrap();
    let got: Vec<String> = set.rows.iter().map(|r| r[0].text()).collect();
    let want: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
    assert_eq!(got, want, "X2 before X10");
    let set = reader
        .query("SELECT ew_ml(title_ml, 'en'), title FROM api_v1_page WHERE folio = '101'")
        .unwrap();
    assert_eq!(set.rows[0][0], set.rows[0][1], "ew_ml reads the page title");
}

#[test]
fn two_builds_write_the_same_image() {
    let (a, b) = (projection(), projection());
    assert!(a.image() == b.image(), "the projection image is deterministic");
    assert!(a.image().starts_with(b"SQLite format 3\0"));
}

#[test]
fn messages_carry_the_check_run() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader
        .query("SELECT rule, severity, state, target_tags FROM api_v1_message WHERE rule = 'EW-CORE-033'")
        .unwrap();
    // -3W1 has no length (doc 20 §10, EW-CORE-033).
    assert_eq!(set.rows.len(), 1);
    assert_eq!(set.rows[0][1].text(), "error");
    assert!(set.rows[0][3].text().ends_with("-3W1"));
}
