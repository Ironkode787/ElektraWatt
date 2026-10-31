// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Derivation findings of the Session (review cycle 6 B, D1; ADR-0003): an unknown pinned profile is
//! surfaced on load as `EW-CORE-018` at error severity; the full pass adds the notes of deriving tags and
//! frames.

use ew_api::{AnyCommand, DeriveNote, NoteSeverity, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;

fn session(project: &str) -> Session {
    let project = serde_json::from_str(project).unwrap();
    Session::from_records(
        project,
        [],
        [],
        SessionOptions {
            ids: Some(seeded_ids(7)),
            ..SessionOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn an_unknown_profile_is_an_error_finding_on_load() {
    let s = session(r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2099.1"}"#);
    let findings = s.load_findings();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].code, Some("EW-CORE-018"));
    assert_eq!(findings[0].severity, NoteSeverity::Error);
    assert!(findings[0].message.contains("2099.1"));
    assert!(matches!(findings[0].note, DeriveNote::UnknownProfile { .. }));
    // The full pass repeats it.
    assert!(s.derivation_findings().contains(&findings[0]));
}

#[test]
fn a_pinned_profile_and_valid_settings_give_no_load_finding() {
    let s = session(r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#);
    assert!(s.load_findings().is_empty());
    assert!(s.derivation_findings().is_empty());
}

#[test]
fn malformed_settings_members_are_warnings_on_load() {
    let s = session(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1",
            "settings":{"numbering":7,"frames":"x"}}"#,
    );
    let members: Vec<String> = s
        .load_findings()
        .iter()
        .map(|f| {
            assert_eq!(f.severity, NoteSeverity::Warning);
            assert_eq!(f.code, None);
            match &f.note {
                DeriveNote::MalformedShim { member, .. } => member.clone(),
                other => panic!("unexpected {other:?}"),
            }
        })
        .collect();
    assert_eq!(members, ["settings.frames", "settings.numbering"]);
}

#[test]
fn the_full_pass_notes_the_default_frame_of_the_current_snapshot() {
    let mut s = session(r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#);
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": 1, "title": "Cover"}),
    })
    .unwrap();
    assert!(s.load_findings().is_empty());
    let findings = s.derivation_findings();
    assert!(
        findings
            .iter()
            .any(|f| f.note == DeriveNote::DefaultFrame && f.severity == NoteSeverity::Info),
        "{findings:?}"
    );
}
