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
