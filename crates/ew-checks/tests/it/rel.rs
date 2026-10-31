// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-REL and the release gate (doc 15 §4.8, §5.4, §5.5; WP-S1.18 part 3, D9, D10).

use crate::common;

use common::{model, model_with, record, us};
use ew_checks::{
    CheckMode, FULL, Finding, GateInput, PRE_DELIVERY, RuleKind, blockers, gate, pack_definition,
    pack_versions, rule, run,
};
use ew_derive::Q;
use ew_derive::ew_schema::{Record, Sha256};
use serde_json::{Value, json};

/// A revision record `n` with the members of `extra`.
fn revision(n: u128, label: &str, state: &str, extra: Value) -> Record {
    let mut v = json!({"$k": "revision", "id": us(n), "label": label, "state": state});
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    record(v)
}

fn digest(n: u8) -> Sha256 {
    Sha256([n; 32])
}

fn rel(findings: &[Finding]) -> Vec<(&'static str, String, String)> {
    findings
        .iter()
        .filter(|f| f.rule.starts_with("EW-REL"))
        .map(|f| (f.rule, f.discriminator.clone(), f.message.clone()))
        .collect()
}

fn gated(records: Vec<Record>, content_digest: Option<Sha256>) -> Vec<(&'static str, String, String)> {
    let m = model(records);
    let q = Q::new(&m);
    rel(&gate(&q, &PRE_DELIVERY, &GateInput { content_digest }).findings)
}

/// The EW-REL rules are gate-only integrity rules of pack EW-REL (doc 15 §4.8): no scheme runs them online or in a
/// plain release-mode run, so `ew check` never reports "checker not set" (doc 15 §3.2); the gate does.
#[test]
fn ew_rel_runs_in_the_gate_only() {
    for id in ["EW-REL-001", "EW-REL-002", "EW-REL-004"] {
        let m = rule(id).expect("registered").meta();
        assert_eq!(m.pack, "EW-REL");
        assert_eq!(m.kind, RuleKind::Integrity);
        assert!(!m.waivable(), "{id}");
        assert!(m.modes.gate, "{id}");
        for s in ew_checks::SCHEMES {
            assert!(!s.enabled(m, CheckMode::Online), "{id} {}", s.name);
            assert!(!s.enabled(m, CheckMode::Release), "{id} {}", s.name);
        }
    }
    let records = vec![revision(1, "V1.0", "open", json!({"author": "engineer"}))];
    let m = model(records.clone());
    let q = Q::new(&m);
    assert!(rel(&run(&q, &FULL, CheckMode::Release).findings).is_empty());
    assert_eq!(
        gated(records, Some(digest(1))),
        [(
            "EW-REL-001",
            "checker".to_owned(),
            "revision V1.0: checker not set; the checker approves the revision".to_owned()
        )]
    );
}

/// EW-REL-001's conditions (FR-REV-005, FR-REV-021): author and checker not set, a sign-off missing or incomplete, a sign-off over other content; a matching sign-off reports nothing, and a project
/// without an open revision has nothing to judge.
#[test]
fn four_eyes_cases() {
    let signed = |d: u8| {
        json!({"author": "engineer", "checker": "checker", "sign_off": {"checker": "checker",
        "content_digest": digest(d).to_string()}})
    };
    let disc = |rows: Vec<(&'static str, String, String)>| -> Vec<String> {
        rows.into_iter().map(|(_, d, _)| d).collect()
    };
    assert_eq!(
        disc(gated(
            vec![revision(1, "V1.0", "open", json!({}))],
            Some(digest(1))
        )),
        ["author", "checker"]
    );
    assert_eq!(
        disc(gated(
            vec![revision(1, "V1.0", "open", json!({"checker": "checker"}))],
            Some(digest(1))
        )),
        ["author"]
    );
    // The checker equal to the author is refused by the model (EW-INV-018) and by `revision.approve`; the rule's
    // `four-eyes` condition guards a snapshot that bypassed both, so no loadable model reaches it here.
    assert_eq!(
        disc(gated(
            vec![revision(
                1,
                "V1.0",
                "open",
                json!({"author": "engineer", "checker": "checker"})
            )],
            Some(digest(1))
        )),
        ["sign-off"],
        "a checker without a sign-off (a hand-edited file)"
    );
    assert_eq!(
        disc(gated(
            vec![revision(
                1,
                "V1.0",
                "open",
                json!({"author": "engineer", "checker": "checker", "sign_off": {"checker": "checker"}})
            )],
            Some(digest(1))
        )),
        ["sign-off"],
        "a sign-off without its digest is incomplete"
    );
    assert!(gated(vec![revision(1, "V1.0", "open", signed(1))], Some(digest(1))).is_empty());
    let stale = gated(vec![revision(1, "V1.0", "open", signed(1))], Some(digest(2)));
    assert_eq!(
        stale,
        [(
            "EW-REL-001",
            "stale".to_owned(),
            "revision V1.0: the sign-off does not match the saved content (edited after approval); approve it again"
                .to_owned()
        )]
    );
    // Without a saved generation's digest (`Rule::run`) the digest is not compared.
    assert!(gated(vec![revision(1, "V1.0", "open", signed(1))], None).is_empty());
    // Nothing open: nothing to release.
    assert!(gated(vec![revision(1, "V1.0", "released", json!({}))], Some(digest(1))).is_empty());
}

/// EW-REL-004's gate half (FR-REV-002): a label out of the project's format, and an open revision whose label does not
/// follow every other; a withdrawal record repeats its label and is passed over; a newer build's format is not judged.
#[test]
fn label_order_cases() {
    let ok = vec![
        revision(1, "V1.0", "released", json!({})),
        revision(
            2,
            "V1.0",
            "withdrawn",
            json!({"withdraws": us(1), "reason": "issued in error"}),
        ),
        revision(3, "V2.0", "open", json!({"author": "engineer"})),
    ];
    let only = |rows: Vec<(&'static str, String, String)>| -> Vec<(String, String)> {
        rows.into_iter()
            .filter(|(r, _, _)| *r == "EW-REL-004")
            .map(|(_, d, m)| (d, m))
            .collect()
    };
    assert!(only(gated(ok, Some(digest(1)))).is_empty());
    let bad = vec![
        revision(1, "V3.0", "released", json!({})),
        revision(2, "2", "released", json!({})),
        revision(3, "V2.0", "open", json!({"author": "engineer"})),
    ];
    assert_eq!(
        only(gated(bad.clone(), Some(digest(1)))),
        [
            (
                "format".to_owned(),
                "revision 2: the label is not in the project's revision label format V{major}.{minor}"
                    .to_owned()
            ),
            (
                "order".to_owned(),
                format!(
                    "revision V2.0: the open revision's label does not follow V3.0 of revision {}; revision labels \
                     increase, the open revision last",
                    us(1)
                )
            ),
        ]
    );
    let m = model_with(json!({"revision_label": "base36"}), bad);
    let q = Q::new(&m);
    let report = gate(&q, &PRE_DELIVERY, &GateInput::default());
    assert!(
        !report.findings.iter().any(|f| f.rule == "EW-REL-004"),
        "a label format of a newer build is not judged"
    );
}

/// The gate is the release scheme's run plus EW-REL: its findings are the run's and the EW-REL ones, its rules list the
/// EW-REL rules, its counts count both, and its blockers are the open errors.
#[test]
fn the_gate_joins_the_release_run_and_ew_rel() {
    let m = model(vec![
        common::strip(10, 1),
        common::terminal(11, 10, 1, "1"),
        common::terminal(12, 10, 2, "1"),
        revision(1, "V1.0", "open", json!({"author": "engineer"})),
    ]);
    let q = Q::new(&m);
    let plain = run(&q, &PRE_DELIVERY, CheckMode::Release);
    let g = gate(
        &q,
        &PRE_DELIVERY,
        &GateInput {
            content_digest: Some(digest(1)),
        },
    );
    let without: Vec<&Finding> = g
        .findings
        .iter()
        .filter(|f| !f.rule.starts_with("EW-REL"))
        .collect();
    assert_eq!(without, plain.findings.iter().collect::<Vec<_>>());
    assert_eq!(g.counts.errors, plain.counts.errors + 1);
    for id in ["EW-REL-001", "EW-REL-002", "EW-REL-004"] {
        assert!(g.rules.iter().any(|r| r.id == id), "{id}");
    }
    let b: Vec<&str> = blockers(&g).iter().map(|f| f.rule).collect();
    assert!(b.contains(&"EW-TERM-002") && b.contains(&"EW-REL-001"), "{b:?}");
    assert_eq!(g.exit_code(), 1);
}

/// The pack versions of a run (doc 15 §3.9): every evaluated pack once, in ID order, at the build's version, with the
/// SHA-256 of a definition that names every rule of the pack and its data tables; stable across calls.
#[test]
fn pack_versions_name_every_evaluated_pack() {
    let m = model(vec![revision(1, "V1.0", "open", json!({}))]);
    let q = Q::new(&m);
    let g = gate(&q, &PRE_DELIVERY, &GateInput::default());
    let packs = pack_versions(&g);
    let ids: Vec<&str> = packs.iter().map(|p| p.id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(ids, sorted);
    for p in ["EW-CORE", "EW-POT", "EW-REL", "EW-TERM"] {
        assert!(ids.contains(&p), "{p}: {ids:?}");
    }
    assert!(packs.iter().all(|p| p.version == env!("CARGO_PKG_VERSION")));
    assert_eq!(packs, pack_versions(&g));
    let pot = pack_definition("EW-POT").expect("shipped");
    assert!(pot.contains("rule EW-POT-006 ") && pot.contains("data data/pe_sizing.toml "));
    assert!(
        pack_definition("EW-REL")
            .expect("shipped")
            .contains("rule EW-REL-004 190004 integrity pg ")
    );
    assert!(pack_definition("ACME-X").is_none());
    let distinct: std::collections::BTreeSet<[u8; 32]> = packs.iter().map(|p| p.sha256).collect();
    assert_eq!(distinct.len(), packs.len(), "each pack its own definition");
}

/// FX38a n1: the definition text a sign-off's pack SHA-256 binds names a rule's kind and scope by the catalogue's
/// words (`rules.json`), never by `Debug`, so the digest does not move with a Rust spelling.
#[test]
fn the_pack_definition_text_is_pinned() {
    let text = pack_definition("EW-REL").expect("shipped");
    let want = format!(
        "pack EW-REL {}\n\
         rule EW-REL-001 190001 integrity g error - project MVP [] Four eyes\n\
         rule EW-REL-002 190002 integrity g error - project MVP [] Stale committed computation\n\
         rule EW-REL-004 190004 integrity pg error - project MVP [] Revision label order\n",
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(text, want);
    // The words are the catalogue's for every rule of every pack.
    for r in ew_checks::rules() {
        let m = r.meta();
        assert_eq!(m.kind.name(), format!("{:?}", m.kind).to_lowercase(), "{}", m.id);
        assert_eq!(
            m.scope.name(),
            format!("{:?}", m.scope).to_lowercase(),
            "{}",
            m.id
        );
    }
}
