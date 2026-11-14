// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A fluid line continued across pages (FR-FLU-030, FR-FLU-031, doc 11 §5.1) with the shipped content: a valve's
//! working line ends at the fluid interruption point `ew.iso.ip` on one page, `ip.continue` gives its partner the
//! incoming form on another, and the line from there to the cylinder is one tube in the tube list and the SQL view;
//! a fluid command's length is one undo step.

use std::collections::BTreeMap;

use ew_api::output::{RowExport, row_export};
use ew_api::sql::Profile;
use ew_api::{AnyCommand, NewProjectKind, Origin, ProjectTemplate, Session, SessionOptions};
use ew_schema::Kind;
use serde_json::{Value, json};

/// 7.9375 mm: three steps of the page's connection grid.
const STEP: i64 = 7_937_500;

fn run(s: &mut Session, name: &str, params: Value) -> ew_api::Receipt {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn created(r: &ew_api::Receipt, kind: Kind) -> String {
    r.created
        .iter()
        .find(|k| k.0 == kind)
        .map(|k| k.1.to_string())
        .unwrap_or_else(|| panic!("a new {kind:?}"))
}

/// Places the starter symbol `key` on `page` at (`x`, `y`) steps as a new device; its function.
fn place(s: &mut Session, page: &str, key: &str, x: i64, y: i64) -> String {
    let copies = s
        .starter_embed(key)
        .map_or_else(Vec::new, |c| c["copies"].as_array().cloned().unwrap_or_default());
    let symbol = s.starter_symbol_copy(key).expect("a starter symbol");
    let r = run(
        s,
        "device.place",
        json!({"page": page, "symbol": symbol.to_string(), "pos": [x * STEP, y * STEP],
            "target": {"new_device": {"letter": "Q"}}, "copies": copies}),
    );
    created(&r, Kind::Function)
}

fn fluid_line(s: &mut Session, page: &str, a: (&str, u16), b: (&str, u16)) -> String {
    let r = run(
        s,
        "connection.draw",
        json!({"a": {"cp": {"function": a.0, "idx": a.1}}, "b": {"cp": {"function": b.0, "idx": b.1}},
            "page": page, "kind": {"fluid": {}}}),
    );
    r.outcome.primary.expect("the line").1.to_string()
}

/// The cells of a CSV text by header, one map per data row.
fn csv_rows(csv: &str) -> Vec<BTreeMap<String, String>> {
    let mut lines = csv.lines();
    let header: Vec<String> = lines
        .next()
        .unwrap_or_default()
        .split(',')
        .map(str::to_owned)
        .collect();
    lines
        .map(|l| {
            header
                .iter()
                .cloned()
                .zip(l.split(',').map(str::to_owned))
                .collect()
        })
        .collect()
}

fn tube_list(s: &Session) -> Vec<BTreeMap<String, String>> {
    let tubes = row_export(&s.query(), &RowExport::Kind("tube_list".into()), None).expect("the tube list");
    csv_rows(&tubes.csv)
}

#[test]
fn a_fluid_line_continued_across_pages_is_one_tube() {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(0x1030)),
        ..SessionOptions::default()
    };
    let mut s = Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap();
    let mut pages = Vec::new();
    for number in [201, 202] {
        let r = run(&mut s, "page.create", json!({"kind": "fluid", "number": number}));
        pages.push(r.outcome.primary.expect("the new page").1.to_string());
    }
    let valve = place(&mut s, &pages[0], "ew.iso.valve-5-2-sol-spring@qet10", 12, 12);
    // The outgoing form on the valve's page, named F1.
    if let Some(embed) = s.starter_embed("ew.iso.ip-target@qet10") {
        run(&mut s, "library.embed", embed);
    }
    let copies = s
        .starter_embed("ew.iso.ip@qet10")
        .map_or_else(Vec::new, |c| c["copies"].as_array().cloned().unwrap_or_default());
    let ip = s
        .starter_symbol_copy("ew.iso.ip@qet10")
        .expect("the fluid interruption point");
    let r = run(
        &mut s,
        "ip.place",
        json!({"page": pages[0], "symbol": ip.to_string(), "pos": [30 * STEP, 12 * STEP], "name": "F1",
            "direction": "source", "copies": copies}),
    );
    let source = r.outcome.primary.expect("the point").1.to_string();
    fluid_line(&mut s, &pages[0], (&valve, 1), (&source, 0));
    // Its partner on the other page takes the incoming form; the line goes on to the cylinder.
    let r = run(
        &mut s,
        "ip.continue",
        json!({"ip": source, "page": pages[1], "pos": [6 * STEP, 12 * STEP]}),
    );
    let target = r.outcome.primary.expect("the partner").1.to_string();
    let target_symbol = s
        .query()
        .reps()
        .iter()
        .find(|(_, r)| r.functions.iter().any(|f| f.to_string() == target))
        .map(|(_, r)| r.symbol.as_uuid());
    assert_eq!(target_symbol, s.starter_symbol_copy("ew.iso.ip-target@qet10"));
    let cylinder = place(&mut s, &pages[1], "ew.iso.cylinder-double-acting@qet10", 24, 12);
    let line = fluid_line(&mut s, &pages[1], (&target, 0), (&cylinder, 0));

    let rows = tube_list(&s);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(
        rows[0]["From"].ends_with(":2") && rows[0]["To"].ends_with(":A"),
        "{rows:?}"
    );
    let projection = s.sql_projection(None).expect("the projection builds");
    let reader = projection.reader(Profile::EW_SQL).unwrap();
    let set = reader
        .query("SELECT from_port, to_port, from_role, to_role FROM api_v1_fluid_line")
        .unwrap();
    let cells: Vec<Vec<String>> = set
        .rows
        .iter()
        .map(|r| r.iter().map(|c| c.text()).collect())
        .collect();
    assert_eq!(cells, [["2", "A", "working2", "consumer"]]);

    // A fluid command is one undo step.
    run(
        &mut s,
        "fluid.length.set",
        json!({"lines": [line], "length": 1_500_000_000}),
    );
    assert_eq!(
        tube_list(&s)[0]["Source"],
        "missing",
        "the other line has no length yet"
    );
    s.undo(&Origin::Gui).expect("undo");
    s.redo(&Origin::Gui).expect("redo");
    let source_line: Vec<String> = {
        let q = s.query();
        q.connections()
            .iter()
            .filter(|(id, c)| id.to_string() != line && matches!(c.kind, ew_schema::ConnectionKind::Fluid(_)))
            .map(|(id, _)| id.to_string())
            .collect()
    };
    run(
        &mut s,
        "fluid.length.set",
        json!({"lines": source_line, "length": 500_000_000}),
    );
    let rows = tube_list(&s);
    assert_eq!(
        (rows[0]["Length"].as_str(), rows[0]["Source"].as_str()),
        ("2 m", "manual")
    );
    s.undo(&Origin::Gui).expect("undo");
    assert_eq!(tube_list(&s)[0]["Source"], "missing");
}
