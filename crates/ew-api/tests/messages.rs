// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel's provider on Golden A (WP-S2.09 part 2, the `ew-api` half of acceptance check MS1): the finding
//! rows equal `Session::check(online_scheme, Online)`'s findings in order, each under its severity and its rule; EF-08
//! (EW-CORE-033 on -3W1) is an error row that opens the page of -3W1's first drawable with it selected; the keys are
//! stable across a revision that changes no finding; the online scheme is `drafting` while the project names none.

use std::collections::BTreeSet;
use std::path::Path;

use ew_api::checks::{CheckMode, Finding};
use ew_api::output::{NavKind, NavNode, NavRole, NavTable};
use ew_api::{AnyCommand, Session};
use serde_json::json;

/// Golden A's fixture, read-only (no lock; commands run in memory).
fn golden() -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    Session::open_read_only(&dir, "ElektraWatt test")
        .expect("the golden fixture opens")
        .0
}

fn parent<'t>(t: &'t NavTable, n: &NavNode) -> &'t NavNode {
    &t.nodes[n.parent.expect("a parent") as usize]
}

fn findings(t: &NavTable) -> Vec<&NavNode> {
    t.nodes.iter().filter(|n| n.role == NavRole::Finding).collect()
}

fn keys(t: &NavTable) -> Vec<String> {
    t.nodes.iter().map(|n| n.key.clone()).collect()
}

/// The order the provider promises: severity Error, Warning, Info; within one, rule ID; within one rule, the report's
/// order.
fn provider_order(report: &[Finding]) -> Vec<String> {
    let mut out = Vec::new();
    for sev in ["error", "warning", "info"] {
        let of: Vec<&Finding> = report.iter().filter(|f| f.severity.name() == sev).collect();
        let rules: BTreeSet<&str> = of.iter().map(|f| f.rule).collect();
        for r in rules {
            out.extend(
                of.iter()
                    .filter(|f| f.rule == r)
                    .map(|f| f.fingerprint.to_string()),
            );
        }
    }
    out
}

#[test]
fn ms1_the_rows_are_the_online_findings_under_their_severity_and_rule() {
    let s = golden();
    assert_eq!(
        s.online_scheme().name,
        "drafting",
        "Golden A names no active scheme"
    );
    let report = s.check(s.online_scheme(), CheckMode::Online);
    assert!(!report.findings.is_empty());
    let t = s.navigator_table(NavKind::Messages);
    assert_eq!(t.kind, NavKind::Messages);
    let keys_set: BTreeSet<&str> = t.nodes.iter().map(|n| n.key.as_str()).collect();
    assert_eq!(keys_set.len(), t.nodes.len(), "keys are unique");
    // Every finding once, in the provider's order, each one a table row under its rule under its severity.
    let rows = findings(&t);
    assert_eq!(rows.len(), report.findings.len());
    let got: Vec<String> = rows
        .iter()
        .map(|n| n.key.rsplit("finding:").next().unwrap().to_owned())
        .collect();
    assert_eq!(got, provider_order(&report.findings));
    for (n, f) in rows.iter().map(|n| {
        let fp = n.key.rsplit("finding:").next().unwrap();
        (
            *n,
            report
                .findings
                .iter()
                .find(|f| f.fingerprint.to_string() == fp)
                .unwrap(),
        )
    }) {
        assert!(n.table_row);
        let rule = parent(&t, n);
        assert_eq!(rule.role, NavRole::Rule);
        assert!(rule.label.starts_with(f.rule), "{}", rule.label);
        assert_eq!(
            rule.key,
            format!("severity:{}/rule:{}", f.severity.name(), f.rule)
        );
        let sev = parent(&t, rule);
        assert_eq!(sev.role, NavRole::Severity);
        assert_eq!(sev.parent, None);
        assert_eq!(n.cells[0], f.severity.name());
        assert_eq!(n.cells[1], f.rule);
        assert_eq!(n.cells[2], f.message);
        assert_eq!(n.cells[3], f.subject);
        assert_eq!(n.badges[0], f.severity.name());
        assert_eq!(n.badges.contains(&"fix".to_owned()), f.fix.is_some());
        let targets: Vec<_> = f.targets.iter().map(|t| t.id.as_uuid()).collect();
        assert_eq!(n.records, targets);
        assert!(n.facets.iter().any(|(k, _)| k == "kind"));
        assert!(
            n.facets
                .iter()
                .any(|(k, v)| k == "state" && (v == "open" || v == "waived"))
        );
    }
    // The rule group's badge counts its findings.
    for rule in t.nodes.iter().filter(|n| n.role == NavRole::Rule) {
        let at = t.nodes.iter().position(|n| n.key == rule.key).unwrap();
        let n = t
            .nodes
            .iter()
            .filter(|x| x.parent == Some(u32::try_from(at).unwrap()))
            .count();
        assert_eq!(rule.badges, [n.to_string()]);
    }
    // The severity groups: Errors, Warnings, Infos, the empty ones left out.
    let groups: Vec<&str> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Severity)
        .map(|n| n.label.as_str())
        .collect();
    let expected: Vec<&str> = [("error", "Errors"), ("warning", "Warnings"), ("info", "Infos")]
        .into_iter()
        .filter(|(w, _)| report.findings.iter().any(|f| f.severity.name() == *w))
        .map(|(_, l)| l)
        .collect();
    assert_eq!(groups, expected);
    let columns: Vec<&str> = t.columns.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        columns,
        [
            "severity",
            "rule",
            "message",
            "object",
            "page_cell",
            "fix",
            "state"
        ]
    );
    assert!(t.columns.iter().all(|c| c.derived && c.route.is_none()));
    assert_eq!(
        t.columns
            .iter()
            .filter(|c| c.key_column)
            .map(|c| c.key.as_str())
            .collect::<Vec<_>>(),
        ["rule"]
    );
}

#[test]
fn ms1_ef08_is_an_error_row_that_opens_the_cable_on_its_page() {
    let s = golden();
    let t = s.navigator_table(NavKind::Messages);
    let row = findings(&t)
        .into_iter()
        .find(|n| n.cells[1] == "EW-CORE-033" && n.cells[3].ends_with("-3W1"))
        .expect("EF-08: EW-CORE-033 on -3W1");
    assert!(row.key.starts_with("severity:error/rule:EW-CORE-033/finding:"));
    assert_eq!(row.badges[0], "error");
    assert!(row.label.starts_with(&row.cells[3]), "{}", row.label);
    assert!(
        row.facets
            .contains(&("kind".to_owned(), "completeness".to_owned()))
    );
    // Enter opens the page of -3W1's first drawable with its drawables there selected.
    let first = row.drawables.first().expect("-3W1 is drawn");
    let open = row.open.as_ref().expect("an open target");
    assert_eq!(open.page, first.page);
    let on_page: Vec<_> = row
        .drawables
        .iter()
        .filter(|d| d.page == first.page)
        .map(|d| d.record)
        .collect();
    assert_eq!(open.select, on_page);
    assert!(open.select.contains(&first.record));
    assert_eq!(row.cells[4], format!("{}-{}", first.folio, first.cell));
    assert!(
        row.facets
            .contains(&("page".to_owned(), first.folio.to_lowercase()))
    );
}

#[test]
fn ms1_keys_are_stable_across_a_revision_that_changes_no_finding() {
    let mut s = golden();
    let before = s.navigator_table(NavKind::Messages);
    let devices = s.navigator_table(NavKind::Devices);
    let dev = devices
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Device && n.label == "-201U3")
        .expect("-201U3");
    let set = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [dev.records[0].to_string()], "path": "function_text",
            "value": "Changed"}),
    };
    s.execute(&set).expect("a function text");
    let after = s.navigator_table(NavKind::Messages);
    assert_ne!(before.rev, after.rev);
    assert_eq!(keys(&before), keys(&after));
    // Determinism: two builds are equal.
    assert_eq!(after, s.navigator_table(NavKind::Messages));
}

#[test]
fn the_online_scheme_is_the_projects_active_scheme_else_drafting() {
    let mut s = golden();
    assert_eq!(s.online_scheme().name, "drafting");
    let set = AnyCommand {
        name: "project.checks.set".into(),
        params: json!({"active_scheme": "full"}),
    };
    s.execute_with(&set, ew_api::Submit::from(ew_api::Origin::Gui))
        .expect("the active scheme");
    assert_eq!(s.online_scheme().name, "full");
    // The panel runs it: its rows are the full scheme's findings.
    let report = s.check(s.online_scheme(), CheckMode::Online);
    assert_eq!(
        findings(&s.navigator_table(NavKind::Messages)).len(),
        report.findings.len()
    );
}
