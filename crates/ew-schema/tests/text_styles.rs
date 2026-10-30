// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ProjectSettings.text_styles` (ADR-0032, an optional member of the unreleased 0.1 under ADR-0011 amendment 4): a
//! project line without it is unchanged, and a line with it, an unknown member inside an entry or a weight of a newer
//! format reads and writes back byte for byte (WP-S1.12 text styles, TS-1 and TS-5).

mod common;

use common::fill;
use common::lines::GOLDEN;
use ew_schema::{Record, TextWeight, effective_text_styles};
use ew_units::Nm;

/// The golden project line with `member` inserted after `formatting`, the member before `text_styles`.
fn project_with(member: &str) -> (String, String) {
    let template = GOLDEN
        .iter()
        .find(|t| t.contains(r#""$k":"project""#))
        .expect("a project line");
    let base = fill(template);
    let anchor = r#""decimal_separator":"point"}"#;
    assert_eq!(base.matches(anchor).count(), 1);
    let with = base.replacen(anchor, &format!("{anchor},{member}"), 1);
    (base, with)
}

fn round_trip(line: &str) -> Record {
    let record: Record = serde_json::from_str(line).unwrap();
    assert_eq!(serde_json::to_string(&record).unwrap(), line);
    record
}

#[test]
fn a_project_line_without_text_styles_is_unchanged() {
    let (base, _) = project_with("");
    let Record::Project(p) = round_trip(&base) else {
        panic!("a project");
    };
    assert!(p.settings.text_styles.is_empty());
    assert_eq!(
        effective_text_styles(&p.settings).len(),
        ew_schema::BUILTIN_TEXT_STYLES.len()
    );
}

#[test]
fn text_styles_with_unknown_members_and_weights_round_trip() {
    let member = r#""text_styles":{"cp.small":{"size":1852083},"marker":{"size":1852083,"weight":"black","family":"mono"},"note.big":{"size":6000000,"weight":"bold","zz_newer":{"a":1}}}"#;
    let (_, with) = project_with(member);
    let Record::Project(p) = round_trip(&with) else {
        panic!("a project");
    };
    let styles = &p.settings.text_styles;
    assert_eq!(styles.len(), 3);
    assert!(matches!(styles["marker"].weight, Some(TextWeight::Unknown(_))));
    assert_eq!(styles["marker"].extra.len(), 1);
    assert_eq!(styles["note.big"].extra.len(), 1);
    let rows = effective_text_styles(&p.settings);
    let get = |k: &str| rows.iter().find(|r| r.key.as_str() == k).unwrap();
    assert_eq!(get("marker").em, Nm(1_852_083));
    assert_eq!(
        get("marker").weight,
        TextWeight::Regular,
        "an unknown weight reads as absent"
    );
    assert_eq!(get("note.big").weight, TextWeight::Bold);
}

#[test]
fn an_empty_member_is_not_written_and_keys_are_identifiers() {
    // `{}` reads as no overrides and writes nothing (the member is skipped when empty).
    let (base, with) = project_with(r#""text_styles":{}"#);
    let record: Record = serde_json::from_str(&with).unwrap();
    assert_eq!(serde_json::to_string(&record).unwrap(), base);
    // A key that is not an identifier and a size that is not an integer are refused by the typed reader.
    for bad in [
        r#""text_styles":{" marker":{"size":1852083}}"#,
        r#""text_styles":{"marker":{"size":1.5}}"#,
        r#""text_styles":{"marker":{}}"#,
    ] {
        let (_, with) = project_with(bad);
        assert!(serde_json::from_str::<Record>(&with).is_err(), "{bad}");
    }
}
