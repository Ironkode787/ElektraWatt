// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Session::navigator_table` on Golden A (WP-S2.06 part 1, acceptance check NV2): the Pages table in the derived page
//! order, the Devices tree with one device row per non-terminal device, -201U3's representation rows (D16), keys
//! stable across a revision that changes nothing they show, and the index from a representation to its rows.

use std::collections::BTreeSet;
use std::path::Path;

use ew_api::output::{NavKind, NavRole, NavTable};
use ew_api::{AnyCommand, Origin, Session};
use serde_json::json;

/// Golden A's fixture, read-only (no lock; commands run in memory).
fn golden() -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    Session::open_read_only(&dir, "ElektraWatt test")
        .expect("the golden fixture opens")
        .0
}

fn keys(t: &NavTable) -> Vec<String> {
    t.nodes.iter().map(|n| n.key.clone()).collect()
}

fn node<'t>(t: &'t NavTable, role: NavRole, label: &str) -> (u32, &'t ew_api::output::NavNode) {
    let i = t
        .nodes
        .iter()
        .position(|n| n.role == role && n.label == label)
        .unwrap_or_else(|| panic!("no {role:?} row {label}"));
    (u32::try_from(i).unwrap(), &t.nodes[i])
}

#[test]
fn pages_lists_the_page_order_under_its_sections() {
    let s = golden();
    let t = s.navigator_table(NavKind::Pages);
    let rows = s.page_rows();
    assert_eq!(rows.len(), 36);
    let pages: Vec<&ew_api::output::NavNode> = t.nodes.iter().filter(|n| n.table_row).collect();
    assert_eq!(pages.len(), 36);
    let folios: Vec<&str> = pages.iter().map(|n| n.label.as_str()).collect();
    let order: Vec<&str> = rows.iter().map(|r| r.folio.as_str()).collect();
    assert_eq!(folios, order, "the derived page order");
    assert_eq!(
        t.columns.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
        ["folio", "title", "type", "frame", "generated"]
    );
    for (n, r) in pages.iter().zip(&rows) {
        assert!(n.key.ends_with(&format!("p:{}", r.id)), "{}", n.key);
        assert_eq!(n.cells[0], r.folio);
        assert_eq!(n.cells[1], r.title);
        // A generated page and a section's divider have no page record.
        assert_eq!(
            n.records.is_empty(),
            r.generated || r.kind == "section_divider",
            "{}",
            r.folio
        );
        assert_eq!(n.open.as_ref().map(|o| o.page), Some(r.id));
        let parent = &t.nodes[n.parent.unwrap() as usize];
        assert_eq!(parent.role, NavRole::Section);
        assert!(parent.key == "s:front" || parent.key.starts_with("s:"));
    }
    let unique: BTreeSet<String> = keys(&t).into_iter().collect();
    assert_eq!(unique.len(), t.nodes.len(), "keys are unique");
}

#[test]
fn devices_lists_every_non_terminal_device_once() {
    let s = golden();
    let t = s.navigator_table(NavKind::Devices);
    let devices: Vec<&ew_api::output::NavNode> =
        t.nodes.iter().filter(|n| n.role == NavRole::Device).collect();
    assert!(devices.iter().all(|n| n.table_row));
    assert_eq!(t.nodes.iter().filter(|n| n.table_row).count(), devices.len());
    let rows = ew_reports_count(&s);
    assert_eq!(
        devices.len(),
        rows,
        "one row per non-terminal device (the structure overview's device rows)"
    );
    let records: BTreeSet<_> = devices.iter().map(|n| n.records[0]).collect();
    assert_eq!(records.len(), devices.len(), "each device once");
    // Natural tag order inside each structure node: siblings of the same parent and role keep natural order.
    for (i, n) in t.nodes.iter().enumerate() {
        if n.role != NavRole::Device {
            continue;
        }
        let next = t.nodes[i + 1..]
            .iter()
            .find(|m| m.parent == n.parent && m.role == NavRole::Device);
        if let Some(m) = next
            && t.nodes[n.parent.unwrap() as usize].role == NavRole::Location
        {
            assert!(n.natural[0] <= m.natural[0], "{} before {}", n.label, m.label);
        }
    }
    assert_eq!(t.columns[0].key, "designation");
    assert!(t.columns[0].key_column);
    let ft = t.columns.iter().find(|c| c.key == "function_text").unwrap();
    assert_eq!(ft.route, Some(("device".to_owned(), "function_text".to_owned())));
    let unique: BTreeSet<String> = keys(&t).into_iter().collect();
    assert_eq!(unique.len(), t.nodes.len(), "keys are unique");
}

/// The number of device rows of the structure overview (its rows minus its structure-node rows).
fn ew_reports_count(s: &Session) -> usize {
    let q = s.query();
    let set = ew_reports::project_rows(&q, "structure_overview", None).unwrap();
    let kind = set.column("kind").unwrap();
    (0..set.rows.len())
        .filter(|&i| ew_reports::csv::row_texts(&q, &set, i)[kind] == "Device")
        .count()
}

#[test]
fn the_201u3_row_has_its_two_representations_and_the_index_finds_them() {
    let s = golden();
    let t = s.navigator_table(NavKind::Devices);
    let (di, dev) = node(&t, NavRole::Device, "-201U3");
    let reps: Vec<&str> = t
        .nodes
        .iter()
        .filter(|n| {
            n.role == NavRole::Representation
                && n.parent
                    .and_then(|f| t.nodes[f as usize].parent)
                    .is_some_and(|d| d == di)
        })
        .map(|n| n.label.as_str())
        .collect();
    assert_eq!(reps, ["201-E4", "203-B4"], "D16");
    let cells: Vec<String> = dev
        .drawables
        .iter()
        .map(|d| format!("{}-{}", d.folio, d.cell))
        .collect();
    assert_eq!(cells, ["201-E4", "203-B4"]);
    assert!(dev.facets.contains(&("page".to_owned(), "201".to_owned())));
    assert_eq!(dev.open.as_ref().map(|o| o.select.len()), Some(1));
    // The index maps a representation to every node that carries it, the deepest last.
    let rep = dev.drawables[0].record;
    let carriers = &t.index[&rep];
    assert_eq!(carriers[0], di);
    let last = &t.nodes[*carriers.last().unwrap() as usize];
    assert_eq!(last.role, NavRole::Representation);
    assert_eq!(last.label, "201-E4");
    assert_eq!(t.index[&dev.records[0]], [di]);
}

#[test]
fn keys_are_stable_across_a_revision() {
    let mut s = golden();
    let before = s.navigator_table(NavKind::Devices);
    let pages_before = s.navigator_table(NavKind::Pages);
    let (_, dev) = node(&before, NavRole::Device, "-201U3");
    let set = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [dev.records[0].to_string()], "path": "function_text",
            "value": "Changed"}),
    };
    s.execute(&set).expect("a function text");
    let after = s.navigator_table(NavKind::Devices);
    assert_ne!(before.rev, after.rev);
    assert_eq!(keys(&before), keys(&after));
    s.undo(&Origin::Gui).expect("undo");
    assert_eq!(keys(&s.navigator_table(NavKind::Pages)), keys(&pages_before));
    // Part 2's kinds have their rows (WP-S2.06 part 2).
    let c = s.navigator_table(NavKind::Connections);
    assert!(!c.nodes.is_empty());
    assert!(!c.columns.is_empty());
    // Determinism: two builds of every kind are equal, and every kind has rows on Golden A.
    for k in NavKind::ALL {
        let a = s.navigator_table(k);
        assert_eq!(a, s.navigator_table(k), "{k:?}");
        assert!(!a.nodes.is_empty(), "{k:?} has rows");
    }
}

/// NFR-PERF-15 at XL (§11): the Devices table of the XL synthetic fixture and a 5-character filter over its tokens.
/// Not run during development (owner's rule, ID-512): the release phase runs it with FT-491 and reads the timings.
#[test]
#[ignore = "release-phase measurement (FT-491); not run during development (owner's rule, ID-512)"]
fn devices_table_and_a_filter_at_xl() {
    let (project, records, lib) =
        ew_testkit::synth::project(ew_testkit::synth::SizeClass::XL, 1).into_parts();
    let s = Session::from_records(project, records, lib, ew_api::SessionOptions::default())
        .expect("the XL fixture is a valid model");
    // The bench helper's clock (ew-testkit) measures the release-phase timing; nothing derived reads it.
    let (t, built) = ew_testkit::bench::time(|| s.navigator_table(NavKind::Devices));
    let query = ew_api::output::nav_tokens("k1001");
    let (hits, filtered) = ew_testkit::bench::time(|| {
        t.nodes
            .iter()
            .filter(|n| {
                query
                    .iter()
                    .all(|q| n.tokens.iter().any(|x| x.starts_with(q.as_str())))
            })
            .count()
    });
    println!(
        "XL Devices table: {} nodes in {built:?}; filter `k1001`: {hits} hits in {filtered:?}",
        t.nodes.len()
    );
    assert!(!t.nodes.is_empty());
}
