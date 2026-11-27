// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-LAY-018 on the sample with its bottom's glands taken off (doc 12 §3.9, doc 15 §3.2): a gland not placed is a
//! warning while drawing and an error at a release, with Place on the bottom; a hole made for a cable that serves none
//! offers Remove the hole, which its command takes.

use crate::common;

use ew_api::AnyCommand;
use ew_api::checks::{CheckMode, PRE_DELIVERY, Severity};
use serde_json::json;

#[test]
fn a_gland_not_placed_is_an_error_at_a_release_and_its_fix_opens_the_bottom() {
    let mut s = common::sample();
    let glands: Vec<String> = s
        .query()
        .table::<ew_schema::SurfaceFeature>()
        .iter()
        .filter(|(_, f)| f.cable.is_some())
        .map(|(id, _)| id.as_uuid().to_string())
        .collect();
    assert!(!glands.is_empty(), "the sample's bottom holds glands");
    let targets: Vec<_> = glands
        .iter()
        .map(|id| json!({"kind": "surface_feature", "id": id}))
        .collect();
    let delete = AnyCommand {
        name: "edit.delete".into(),
        params: json!({ "targets": targets }),
    };
    match s.execute(&delete) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&delete).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect("Delete takes the glands off");
    let of = |mode: CheckMode| -> Vec<ew_api::checks::Finding> {
        s.check(&PRE_DELIVERY, mode)
            .findings
            .into_iter()
            .filter(|f| f.rule == "EW-LAY-018")
            .collect()
    };
    let online = of(CheckMode::Online);
    assert!(!online.is_empty());
    assert!(
        online.iter().all(|f| f.severity == Severity::Warning),
        "{online:#?}"
    );
    let release = of(CheckMode::Release);
    assert!(
        release.iter().all(|f| f.severity == Severity::Error),
        "{release:#?}"
    );
    let fix = release[0].fix.clone().expect("Place on the bottom");
    assert_eq!(fix.command, "layout.surface_page");
    assert!(
        fix.params["surface"]
            .as_str()
            .is_some_and(|v| v.ends_with("/bottom"))
    );
}

/// A cable whose part gives no outer diameter: EW-LAY-018's finding comes from that part's missing outer diameter, whose
/// Edit part… opens the part in Parts at a group its sheet shows, the Cable group where the diameter is typed (doc 12
/// §3.9, doc 07 §5.7).
#[test]
fn edit_part_of_a_cable_without_a_diameter_opens_its_cable_section() {
    let (project, records, mut lib) = common::sample_records();
    for doc in &mut lib {
        if let ew_schema::LibDocument::Part(c) = doc
            && c.item.kind == ew_schema::PartKind::Cable
        {
            c.item.sections.cable = None;
            let kept: Vec<_> = c
                .item
                .tech_data
                .iter()
                .filter(|t| t.key.as_str() != "outer_diameter")
                .cloned()
                .collect();
            c.item.tech_data = ew_schema::KeyedList::from_vec(kept).expect("the other values");
        }
    }
    let s = common::session_of(project, records, lib);
    let lacks: Vec<_> = s
        .check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-LAY-018")
        .filter_map(|f| f.part_lack)
        .collect();
    assert!(
        !lacks.is_empty(),
        "the sample's cables that leave lack a diameter"
    );
    for lack in lacks {
        assert_eq!(lack.field, "sections.cable.outer_diameter");
        let group = ew_api::part_fields::Group::of(lack.field).expect("a declared field");
        let draft = s.part_draft(lack.copy.as_uuid()).expect("the cable's part");
        assert_eq!(draft.text("kind"), "cable");
        assert!(
            draft.sections().contains(&group),
            "the sheet of a cable part has no group {group:?}"
        );
    }
}

/// A cable's Gland and Gland hole rows say how they are had as texts a window renders in its language (architecture
/// §18), not as English it would read back: -103W1's gland is proposed for its 12.5 mm and sits in a hole on page 905.
#[test]
fn a_cable_s_gland_rows_say_how_in_texts() {
    let s = common::sample();
    let cable: ew_ids::Uuid = s.resolve_reference("tag:-103W1").expect("-103W1");
    let sheet = s.property_sheet(&[cable]);
    let via = |key: &str| {
        let row = sheet
            .sections
            .iter()
            .flat_map(|x| &x.rows)
            .find(|r| r.key == key)
            .unwrap_or_else(|| panic!("no row {key}"));
        match &row.provenance {
            ew_api::output::Provenance::Derived { via } => (
                via.key(),
                via.args().iter().map(ToString::to_string).collect::<Vec<_>>(),
            ),
            other => panic!("{key}: {other:?}"),
        }
    };
    assert_eq!(
        via("device/derived.gland"),
        ("via-gland-proposed-for", vec!["12.5".to_owned()])
    );
    assert_eq!(
        via("device/derived.gland_hole"),
        ("via-gland-hole-on-page", vec!["905".to_owned()])
    );
}
