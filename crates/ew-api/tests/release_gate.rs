// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.18 part 3 (D8 to D10): the release gate on the saved generation and the checker's sign-off bound to its
//! content digest, through the `Session` facade (RG-1 to RG-5).
//!
//! Golden A is opened read-only only (RG-4). The approvals run on a **copy** of Golden A in a fresh directory whose
//! release-scheme errors are each waived first (Golden A's EW-POT-006 and EW-POT-013 are safety-kind, so their
//! waivers are the ones a checker acknowledges). Nothing under `golden/` changes.

mod golden_common;

use std::path::{Path, PathBuf};

use ew_api::checks::{self, CheckMode, Finding, PRE_DELIVERY};
use ew_api::{
    AnyCommand, Approval, ApproveError, CmdError, CmdLog, CmdLogError, Exposure, GateError, Origin, Recovery,
    Session, SessionOptions, Submit,
};
use ew_derive::ew_ids::Id;
use ew_schema::{Date, PartyRole, Revision, Waiver};
use ew_store::StoreOptions;
use ew_store::ew_format::{Limits, Manifest, content_digest};

fn golden() -> PathBuf {
    golden_common::repo().join("golden/J-E-2416.ew")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for e in std::fs::read_dir(from).expect("a readable directory") {
        let e = e.expect("an entry");
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).expect("a copied file");
        }
    }
}

fn open(dir: &Path, state: &Path) -> (Session, Vec<checks::IntegrityItem>) {
    let mut options = StoreOptions::new("wp-s1-18-part3");
    options.state_dir = Some(state.to_path_buf());
    let (session, report) = Session::open(dir, options, SessionOptions::default(), Recovery::Keep)
        .unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
    (session, ew_api::integrity_items(&report))
}

/// The content digest of the manifest on disk, computed from the file (D8).
fn digest_on_disk(dir: &Path) -> ew_api::Sha256 {
    let bytes = std::fs::read(dir.join("manifest.json")).expect("manifest.json");
    let manifest = Manifest::parse(&bytes, &Limits::default()).expect("a manifest");
    ew_api::Sha256(content_digest(&manifest).expect("a digest"))
}

fn rel(findings: &[Finding]) -> Vec<(&'static str, String, String)> {
    findings
        .iter()
        .filter(|f| f.rule.starts_with("EW-REL"))
        .map(|f| (f.rule, f.discriminator.clone(), f.message.clone()))
        .collect()
}

fn cmd(name: &str, params: serde_json::Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

fn date() -> Date {
    Date::new(2025, 6, 2).expect("a date")
}

fn approval(checker: PartyRole, waivers_ack: Vec<Id<Waiver>>) -> Approval {
    Approval {
        label: "V2.0".into(),
        checker,
        date: date(),
        waivers_ack,
    }
}

fn open_revision(s: &Session) -> std::sync::Arc<Revision> {
    let q = s.query();
    q.table::<Revision>()
        .iter()
        .find(|(_, r)| r.is_open())
        .map(|(_, r)| std::sync::Arc::clone(r))
        .expect("an open revision")
}

/// RG-4 (D15's refusal half): the gate on Golden A lists exactly the findings of `ew check --scheme pre-delivery --mode
/// release` (with the integrity findings of opening) plus EW-REL-001 "checker not set"; it refuses. The gate runs on a
/// read-only session (no lock); the label must be the open revision's.
#[test]
fn rg4_the_gate_on_golden_a_is_the_release_run_plus_checker_not_set() {
    let (s, report) = Session::open_read_only(&golden(), "wp-s1-18-part3").expect("Golden A opens read-only");
    let integrity = ew_api::integrity_items(&report);
    let gate = s.release_gate("V2.0", &integrity).expect("the gate runs");
    let check = s
        .check(&PRE_DELIVERY, CheckMode::Release)
        .with_integrity(integrity.clone());
    let release: Vec<&Finding> = gate
        .report
        .findings
        .iter()
        .filter(|f| !f.rule.starts_with("EW-REL"))
        .collect();
    assert_eq!(release, check.findings.iter().collect::<Vec<_>>());
    assert_eq!(
        rel(&gate.report.findings),
        [(
            "EW-REL-001",
            "checker".to_owned(),
            "revision V2.0: checker not set; the checker approves the revision (ew revision approve)"
                .to_owned()
        )]
    );
    assert_eq!(gate.report.counts.errors, check.counts.errors + 1);
    assert_eq!(gate.blockers().len(), check.counts.errors as usize + 1);
    assert!(!gate.passes());
    assert_eq!(gate.report.scheme, "pre-delivery");
    assert_eq!(gate.content_digest, digest_on_disk(&golden()));
    assert_eq!(gate.author, Some(PartyRole::Engineer));
    assert!(
        gate.safety_waivers.is_empty(),
        "Golden A waives no safety-kind finding"
    );
    // `ew check` itself never reports EW-REL (doc 15 §3.2).
    assert!(rel(&check.findings).is_empty());
    assert_eq!(
        s.release_gate("V1.0", &integrity),
        Err(GateError::NotTheOpenRevision {
            label: "V1.0".into(),
            open: "V2.0".into()
        })
    );
    s.close();
}

/// D8: the gate and the approval need a saved generation: a session without a project directory has none, and
/// approving needs the write lock.
#[test]
fn the_gate_needs_a_saved_generation_and_approval_the_write_lock() {
    let project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .expect("a project");
    let mut s = Session::from_records(project, [], [], SessionOptions::default()).expect("a session");
    assert_eq!(s.content_digest(), Err(GateError::NoProject));
    assert_eq!(
        s.approve(&approval(PartyRole::Checker, Vec::new()), &[])
            .map(|_| ()),
        Err(ApproveError::Gate(GateError::NoProject))
    );
    let (mut s, _) = Session::open_read_only(&golden(), "wp-s1-18-part3").expect("Golden A opens read-only");
    assert_eq!(
        s.approve(&approval(PartyRole::Checker, Vec::new()), &[])
            .map(|_| ()),
        Err(ApproveError::ReadOnly)
    );
    s.close();
}

/// RG-1, RG-2, RG-3 and RG-5 on a copy of Golden A whose release-scheme errors are waived:
/// - RG-2: approval is refused with unsaved edits, for the author's party, and with the gate's errors (listed);
/// - safety-kind waivers in force must be acknowledged ("unacknowledged safety waivers: …");
/// - RG-1: after a save, the approval writes the checker and the sign-off with the saved manifest's content digest, the
///   scheme, the pack versions and the acknowledged waivers; saving it leaves the digest as it is, and the gate passes;
/// - RG-3: a page-note edit keeps the sign-off valid; a content edit makes EW-REL-001 report it stale;
/// - RG-5: the facade refuses `revision.approve` outside the approval, a script gets the exposure error, and a
///   `.ewcmd` line naming it is refused with the exposure error before any line runs.
#[test]
fn rg1_to_rg5_approve_the_saved_revision_bound_to_its_content() {
    let copy = ew_testdir::fresh_dir("ew-part3-approve");
    let state = ew_testdir::fresh_dir("ew-part3-approve-state");
    copy_dir(&golden(), &copy);
    let (mut s, integrity) = open(&copy, &state);

    // RG-2: the gate's errors besides EW-REL-001 refuse the approval, listed.
    let gate = s.release_gate("V2.0", &integrity).expect("the gate runs");
    let blocked = gate.approval_blockers().len();
    assert!(blocked > 0);
    match s.approve(&approval(PartyRole::Checker, Vec::new()), &integrity) {
        Err(ApproveError::Blocked(list)) => {
            assert_eq!(list.len(), blocked);
            assert!(list.iter().all(|f| f.rule != "EW-REL-001"));
        }
        other => panic!("expected Blocked, got {other:?}"),
    }

    // Waive each of them from the GUI (each is waivable: none is an integrity finding).
    let waive: Vec<(String, String)> = gate
        .approval_blockers()
        .iter()
        .map(|f| (f.rule.to_owned(), f.fingerprint.to_string()))
        .collect();
    for (rule, fingerprint) in &waive {
        s.execute_with(
            &cmd(
                "check.waive",
                serde_json::json!({"rule": rule, "fingerprint": fingerprint, "reason": "accepted for the test release",
                    "author": "engineer", "date": "2025-06-01"}),
            ),
            Submit::from(Origin::Gui),
        )
        .unwrap_or_else(|e| panic!("{rule} waives: {e}"));
    }
    // RG-2: unsaved edits refuse the gate and the approval.
    assert_eq!(s.content_digest(), Err(GateError::Unsaved));
    assert_eq!(
        s.approve(&approval(PartyRole::Checker, Vec::new()), &integrity)
            .map(|_| ()),
        Err(ApproveError::Gate(GateError::Unsaved))
    );
    s.save().expect("the waivers save");

    let gate = s.release_gate("V2.0", &integrity).expect("the gate runs");
    assert!(
        gate.approval_blockers().is_empty(),
        "{:#?}",
        gate.approval_blockers()
    );
    assert_eq!(
        rel(&gate.report.findings).len(),
        1,
        "only EW-REL-001 is left: {:#?}",
        gate.blockers()
    );
    assert_eq!(gate.content_digest, digest_on_disk(&copy));
    // Golden A's safety-kind findings (EW-POT-006, EW-POT-013) are now waived: their waivers are in force.
    let safety: Vec<String> = waive
        .iter()
        .filter(|(rule, _)| checks::rule(rule).is_some_and(|r| r.meta().kind == checks::RuleKind::Safety))
        .map(|(rule, _)| rule.clone())
        .collect();
    assert!(!safety.is_empty());
    assert_eq!(gate.safety_waivers.len(), safety.len());

    // RG-2: the author's party may not check.
    assert_eq!(
        s.approve(
            &approval(PartyRole::Engineer, gate.safety_waivers.clone()),
            &integrity
        )
        .map(|_| ()),
        Err(ApproveError::FourEyes {
            role: PartyRole::Engineer
        })
    );
    // Every safety-kind waiver in force is acknowledged, or the approval is refused with the list.
    let partial = gate.safety_waivers[1..].to_vec();
    let refused = s
        .approve(&approval(PartyRole::Checker, partial), &integrity)
        .map(|_| ())
        .expect_err("one waiver unacknowledged");
    assert_eq!(
        refused,
        ApproveError::Unacknowledged(vec![gate.safety_waivers[0]])
    );
    assert_eq!(
        refused.to_string(),
        format!("unacknowledged safety waivers: {}", gate.safety_waivers[0])
    );
    assert!(open_revision(&s).sign_off.is_none(), "nothing written");

    // RG-1: the approval.
    let (approved, _) = s
        .approve(
            &approval(PartyRole::Checker, gate.safety_waivers.clone()),
            &integrity,
        )
        .expect("approved");
    let r = open_revision(&s);
    assert_eq!(r.checker, Some(PartyRole::Checker));
    let sign = r.sign_off.clone().expect("the sign-off");
    assert_eq!(sign.content_digest, Some(digest_on_disk(&copy)));
    assert_eq!(sign.checker, Some(PartyRole::Checker));
    assert_eq!(sign.date, Some(date()));
    assert_eq!(sign.scheme.as_ref().map(|x| x.as_str()), Some("pre-delivery"));
    let mut ack = gate.safety_waivers.clone();
    ack.sort();
    assert_eq!(sign.waivers_ack, Some(ack));
    let packs: Vec<(String, String, ew_api::Sha256)> = sign
        .packs
        .clone()
        .expect("packs")
        .into_iter()
        .map(|p| {
            (
                p.id.expect("id").as_str().to_owned(),
                p.version.expect("version").as_str().to_owned(),
                p.sha256.expect("sha256"),
            )
        })
        .collect();
    let expected: Vec<(String, String, ew_api::Sha256)> = approved
        .packs
        .iter()
        .map(|p| (p.id.to_owned(), p.version.to_owned(), ew_api::Sha256(p.sha256)))
        .collect();
    assert_eq!(packs, expected);
    assert!(packs.iter().any(|(id, _, _)| id == "EW-REL"));
    // The sign-off lives in revisions.jsonl, which is not content: saving it leaves the digest as it is.
    s.save().expect("the sign-off saves");
    assert_eq!(digest_on_disk(&copy), approved.content_digest);
    let gate = s.release_gate("V2.0", &integrity).expect("the gate runs");
    assert!(gate.passes(), "{:#?}", gate.blockers());

    // RG-5: only the approval submits `revision.approve`.
    let forged = cmd(
        "revision.approve",
        serde_json::json!({"revision": r.id.to_string(), "checker": "checker", "date": "2025-06-02",
            "scheme": "pre-delivery", "content_digest": format!("sha256:{}", "0".repeat(64))}),
    );
    assert!(matches!(s.execute(&forged), Err(CmdError::Rejected { .. })));
    assert!(matches!(s.preview(&forged), Err(CmdError::Rejected { .. })));
    let mcp = Origin::Mcp {
        client: "agent".into(),
    };
    assert!(matches!(
        s.execute_with(&forged, Submit::from(mcp)),
        Err(CmdError::Exposure {
            exposure: Exposure::HumanOnly,
            ..
        })
    ));
    let log = CmdLog::parse(&format!(
        "{{\"$schema\":\"ew.cmdlog/1\"}}\n{{\"cmd\":\"page.create\",\"params\":{{\"number\":990}}}}\n{{\"cmd\":\"revision.approve\",\"params\":{}}}\n",
        forged.params
    ))
    .expect("a command log");
    let before = s.world().rev;
    match s.run_script(&log, "approve.ewcmd") {
        Err(CmdLogError::Refused {
            error: CmdError::Exposure { command, .. },
            ..
        }) => assert_eq!(command, "revision.approve"),
        other => panic!("expected the exposure error, got {other:?}"),
    }
    assert_eq!(s.world().rev, before, "no line ran");

    // RG-3: a page-note edit (revisions.jsonl) keeps the sign-off.
    s.execute(&cmd(
        "fields.set",
        serde_json::json!({"kind": "revision", "targets": [r.id.to_string()], "path": "page_notes", "value": []}),
    ))
    .expect("the page notes change");
    s.save().expect("saved");
    assert_eq!(digest_on_disk(&copy), approved.content_digest);
    assert!(s.release_gate("V2.0", &integrity).expect("gate").passes());
    // RG-3: a content edit voids it.
    let device = s.query().devices().ids().next().expect("a device");
    s.execute(&cmd(
        "fields.set",
        serde_json::json!({"kind": "device", "targets": [device.to_string()], "path": "function_text",
            "value": "edited after approval"}),
    ))
    .expect("a content edit");
    s.save().expect("saved");
    assert_ne!(digest_on_disk(&copy), approved.content_digest);
    let gate = s.release_gate("V2.0", &integrity).expect("the gate runs");
    assert_eq!(
        rel(&gate.report.findings),
        [(
            "EW-REL-001",
            "stale".to_owned(),
            "revision V2.0: the sign-off does not match the saved content (edited after approval); approve it again"
                .to_owned()
        )]
    );
    assert!(!gate.passes());
    s.close();
    let _ = std::fs::remove_dir_all(&copy);
    let _ = std::fs::remove_dir_all(&state);
}
